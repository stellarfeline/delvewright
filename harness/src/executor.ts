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
  Transport,
  Vec3Tuple,
} from "./critical-path.ts";
import { insideCompletion, reachGoal } from "./critical-path.ts";
import type { StepExecutor } from "./sequencer.ts";
import { BotDeathError, formatDeathPos, likelyDeathCause } from "./death.ts";
import { hasSettled } from "./entity-settle.ts";
import { NavigationOwner } from "./navigation.ts";
import type { NamedEntityDeath } from "./teardown.ts";
import type { StageName } from "./report.ts";
import {
  AssistLedger,
  CONTROLLED_GAMEMODE,
  actorAttribution,
  actorExercise,
  actorFloorFinding,
  assistClearCommand,
  assistCommand,
  assistPolicy,
  deathPhases,
  floorFinding,
  checkpointPrecondition,
  giveUpBudgetFor,
  observationOf,
  openTrial,
  unboundedEncounterNote,
  unkillableFinding,
  respawnedAtCheckpoint,
  retryOutcome,
  scriptedDeathCommand,
  scriptedDeathRefusal,
  waveAttribution,
  type ActorEncounter,
  type ActorOutcome,
  type ActorTrial,
  type AssistWindow,
  type CombatPlan,
  type DeathTrial,
  type DeathTrialRecord,
  type Encounter,
  type EncounterPhase,
  type FightAttribution,
  type PerformedRest,
  type ReengageObservation,
  type UnkillableBody,
  type WaveCensus,
} from "./combat.ts";
import {
  bodyInVolume,
  entryCellOf,
  markerAt,
  expectedForfeit,
  inBox,
  openLethalTrial,
  seatAtRespawn,
  tableAnchor,
  type Box,
  type DeathPlan,
  type LethalTrial,
  type StakeRule,
} from "./death-loop.ts";
import { presentAndTrigger } from "./held-item.ts";
import {
  CAMPAIGN_TOKEN,
  markerLine,
  parseCensusMob,
  parseCensusSummary,
  parseCompletionMarker,
  type CensusMob,
  type CensusSummary,
} from "./markers.ts";
import {
  allowNonCollidingEntities,
  configureLeg,
  describeStuckNeighbours,
} from "./movement.ts";
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
  crossingEstimateMs,
  describeGates,
  gateRegionCells,
  gateRetryBudgetMs,
  gateWindowWaitMs,
  gatesCrossedByHop,
  insideGate,
  nearCell,
  needsStandoff,
  openMs,
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
  INTERACTION_REACH,
  acquireFromStances,
  hitboxDims,
  occlusionFailure,
  type Hitbox,
  type Vec3Like,
} from "./crosshair.ts";
import {
  WAVE_CLEAR_STREAK,
  WAVE_ENGAGE_NEAR,
  beginCensusWatch,
  beginWave,
  censusCleared,
  creditsWaveKill,
  observeCensus,
  waveEngagementCleared,
  type WaveCensusWatch,
  type WaveEngagement,
} from "./wave.ts";

/** Bounded number of physics-unstick bursts before a wedged hop fails loudly. */
const UNSTICK_ATTEMPTS = 3;

/**
 * How far (squared horizontal blocks) the bot may sit from its gate staging cell
 * before the station-keep drives it back. 0.4 blocks: tight enough
 * that a window always opens with the bot ON the compiler-pinned mouth, loose
 * enough that the per-poll correction is not a permanent jitter.
 */
const GATE_HOLD_SLACK_SQ = 0.4 * 0.4;

/** Re-aim cadence of the raw crush-gate crossing dash. Faster than
 * the physics would meaningfully change; slow enough not to spam look packets. */
const GATE_DASH_TICK_MS = 50;

/** Squared horizontal arrival radius of the dash at the far mouth cell. */
const GATE_DASH_ARRIVE_SQ = 0.75 * 0.75;

/** Bound on the dash's emergency raw retreat out of the fill (with the corridor
 * current behind it, clearing one or two cells takes a fraction of this). */
const GATE_DASH_RETREAT_MS = 1_500;

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
function legSettledReason(settled: LegSettled | undefined, glabel: string): string | undefined {
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
 * The bot-facing half of a timed-gate crossing, injected by the executor so the
 * control flow above stays unit-testable without a live server.
 */
export interface GateAssist {
  /** The gates the compiler proved this leg's route crosses (empty ⇒ no assist). */
  readonly gates: readonly TimedGate[];
  /**
   * Block (bounded) until `gates` are observed to go from closed to OPEN, so the
   * crossing begins at the top of a window rather than its tail. Returns `true`
   * when the fresh edge was actually OBSERVED and `false` when the wait gave up
   * (region blocks unreadable). On `false` a non-crush caller may still try the
   * hop; a crush-gate caller must NOT — blind entry is the lethal defect.
   *
   * `hold`, when given, is a feet cell the bot must actively KEEP during the wait
   * (a passive idle is not a stance: the tide-mill gate corridor is flowing water,
   * and the current carried the idle bot 8 blocks off the mouth while it watched
   * for the edge — a human holds a movement key against the tide).
   *
   * `press`, when given (crush staging), is the crossing target to LEAN toward
   * whenever the gates provably read CLOSED: the shut gate is a solid wall, so
   * driving at it parks the bot pressed against the plane with forward momentum —
   * and the open edge releases it THROUGH, contact-started. Measured live: from a
   * standing start at the mouth the tide-mill pour is a wall (the wade covered
   * less than 1.5 blocks in the whole 1.8 s window); the press is how a player
   * rides a portcullis. Only applied while the gates read closed — leaning into an
   * OPEN gate outside a fresh window is the blind entry this file forbids.
   */
  readonly waitForWindow: (
    gates: readonly TimedGate[],
    hold?: Vec3Tuple,
    press?: Vec3Tuple,
  ) => Promise<boolean>;
  /** The bot's current feet cell, or `undefined` when it cannot be read. */
  readonly feetCell: () => Vec3Tuple | undefined;
  /**
   * Raw-control crossing burst for a `crush: true` entry: drive
   * straight from the near mouth `from` toward the far mouth `to`, through
   * `through`'s regions, within `budgetMs`; if the budget runs out while still
   * inside a region, retreat raw toward `from` rather than lingering. Returns
   * `true` iff the bot cleared the region on the far side. The pathfinder is the
   * wrong tool for this span (measured live: its start latency plus mid-water
   * replans lost a 1.8 s window and the closing edge killed the bot mid-crossing);
   * a raw look-and-walk is the same mechanism the unstick path and the
   * drowned-bell crossing burst already trust.
   */
  readonly dash?: (
    through: readonly TimedGate[],
    from: Vec3Tuple,
    to: Vec3Tuple,
    budgetMs: number,
  ) => Promise<boolean>;
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
 * closed→open edge, then re-run the hop, escalating to the ordinary stall
 * recovery inside the same window (the pathfinder loses a path whose blocks
 * are rewritten under it; a walk does not). The loop is bounded by
 * {@link gateRetryBudgetMs} (two full cycles + margin) and {@link GATE_MIN_ATTEMPTS};
 * once the budget is spent it makes one final full physical recovery (in case the
 * hop was never about the clock) and then fails loudly, naming the gate and its
 * cycle.
 *
 * This is bounded patience for legs the compiler MARKED, never a blanket retry: an
 * unmarked leg is untouched, and a marked leg that is genuinely unwalkable still
 * fails — the check is not weakened, only told what a gate is.
 *
 * Every retry decision consults the {@link LegSettled} oracle first. A
 * step can complete AT the gate crossing (the tide-mill wheelpit: the objective's
 * distance check fires as the bot lands the crossing, and its emission transports
 * the bot to the next area) — the interrupted hop then looks blocked forever from
 * a cell the one-way transport made unreachable, while the objective the leg
 * exists to reach is already complete. Objective complete ⇒ the crossing
 * SUCCEEDED; no standoff, window wait, or recovery may path the bot back.
 * Returns `"crossed"` when the hop was physically completed and `"settled"` when
 * the oracle ended it — the caller must then stop replaying the WHOLE leg, since
 * its remaining hops belong to the area the transport carried the bot out of.
 *
 * `crush: true` (the portcullis judgement): a crush gate's closing edge
 * KILLS a player caught inside the region, so the discipline above gains three hard
 * rules whenever any staged gate crushes:
 *   1. **no blind entry** — an attempt is made only after the closed→open edge was
 *      actually OBSERVED (`waitForWindow` returning `true`); "crossing anyway" when
 *      the region cannot be read is fine for a gate that merely blocks, and lethal
 *      for one that crushes;
 *   2. **full margin** — the crossing estimate (entry latency + mouth-to-mouth walk,
 *      {@link crossingEstimateMs}) must fit inside the freshly opened window, and
 *      the within-window escalation re-checks the REMAINING window before running;
 *   3. **no post-budget blind recovery** — the final "maybe it was never the clock"
 *      recovery paths straight through the gate at an arbitrary clock position, so
 *      a crush-staged crossing fails loudly instead.
 * The caller (`replayLegWithRecovery`) invokes this PROACTIVELY for a hop whose
 * straight mouth-to-mouth segment crosses a crush gate (`crossing`), staging the
 * entry before the first attempt — the reactive path only ever sees a crush gate
 * after something already went wrong, and with an instant kill there is no
 * afterwards. Checks are not weakened anywhere: refusal to enter still burns the
 * same bounded budget and still ends in the same loud failure.
 */
async function crossTimedGate(
  spec: GoalSpec,
  glabel: string,
  proven: GoalSpec | undefined,
  gate: GateAssist,
  goto: (spec: GoalSpec, label: string) => Promise<void>,
  unstick: Unstick | undefined,
  firstErr: unknown,
  settled?: LegSettled,
  crossing?: readonly TimedGate[],
): Promise<"crossed" | "settled"> {
  const gates = gate.gates;
  // The gates this hop physically crosses (staged wait/standoff target); the whole
  // leg's table when the caller could not tell (reactive path).
  const staged = crossing && crossing.length > 0 ? crossing : gates;
  const lethal = staged.some((g) => g.crush);
  const now = gate.now ?? (() => Date.now());
  const budget = gateRetryBudgetMs(staged);
  const start = now();
  let lastErr = firstErr;
  let attempt = 0;
  const done = (): boolean => legSettledReason(settled, glabel) !== undefined;
  process.stderr.write(
    firstErr === undefined
      ? `[timed-gate] ${glabel}: staged crossing of ${describeGates(staged)} (budget ` +
          `${(budget / 1_000).toFixed(1)}s, min ${GATE_MIN_ATTEMPTS} attempts)\n`
      : `[timed-gate] ${glabel} was interrupted by ${describeGates(staged)}; waiting for a ` +
          `window (budget ${(budget / 1_000).toFixed(1)}s, min ${GATE_MIN_ATTEMPTS} attempts)\n`,
  );
  // The staging cell for a lethal crossing: the last proven waypoint, which for a
  // marked crossing hop is the compiler-pinned gate MOUTH (waypoints.rs
  // `gate_mouth_cells`) — one cell outside the fill, flanking the span DW0378
  // charges. Entry margin is judged from here, so the bot must actually BE here.
  const staging: Vec3Tuple | undefined = proven ? [proven.x, proven.y, proven.z] : undefined;
  while (attempt < GATE_MIN_ATTEMPTS || now() - start < budget) {
    if (done()) return "settled";
    attempt++;
    if (proven && needsStandoff(gate.feetCell(), staged)) {
      process.stderr.write(
        `[timed-gate] standing off to [${proven.x}, ${proven.y}, ${proven.z}] — the bot ` +
          `is inside the gate's fill\n`,
      );
      await reached(() => goto({ ...proven, range: 1 }, `${glabel} gate standoff`));
    }
    // Re-stage a lethal crossing whose bot is off-station (tide-mill: the corridor
    // current carried the idle bot 8 blocks back to the pool between attempts).
    // The mouth is where the margin proof lives; entering from anywhere else is
    // exactly the unproven dash the margin check below refuses.
    if (lethal && proven && staging && !nearCell(gate.feetCell(), staging)) {
      process.stderr.write(
        `[timed-gate] re-staging at the gate mouth [${staging.join(", ")}] — the bot ` +
          `drifted off it\n`,
      );
      await reached(() => goto({ ...proven, range: 1 }, `${glabel} gate re-stage`));
    }
    // Hold the pre-wait stance (post-standoff/re-stage) through the wait: waiting
    // where you stand is the rule (never retreat needlessly), and in a
    // current "where you stand" requires actively standing there. Never hold a
    // cell inside the fill (a range-1 arrival can land one cell into the region;
    // holding there through a crush close is the death itself) — fall back to the
    // proven mouth.
    const feetNow = gate.feetCell();
    const holdCell =
      feetNow && !staged.some((g) => insideGate(feetNow, g)) ? feetNow : staging;
    const observed = await gate.waitForWindow(
      staged,
      holdCell,
      lethal ? [spec.x, spec.y, spec.z] : undefined,
    );
    const openedAt = now();
    // The window wait is long (up to a full cycle) — the settle signal may have
    // landed during it, in which case there is no crossing left to attempt.
    if (done()) return "settled";
    if (lethal && !observed) {
      // Rule 1: a crush gate is never entered on faith. The refusal consumes a
      // bounded wait, so the loop still terminates on the same budget and the
      // failure below still names the gate.
      lastErr = new Error(
        `the gate's closed→open edge could not be observed and ` +
          `${describeGates(staged.filter((g) => g.crush))} crushes — refusing blind entry`,
      );
      process.stderr.write(`[timed-gate] attempt ${attempt}: ${(lastErr as Error).message}\n`);
      continue;
    }
    let crushWindowMs: number | undefined;
    if (lethal) {
      // Rule 2: the crossing must fit the fresh window with margin. The crossing
      // is judged from where the bot actually stands (fallback: the proven mouth),
      // against the SHORTEST open half among the crushing gates.
      const from = gate.feetCell() ?? staging;
      const windowMs = Math.min(...staged.filter((g) => g.crush).map((g) => openMs(g)));
      crushWindowMs = windowMs;
      const estimate = from ? crossingEstimateMs(from, [spec.x, spec.y, spec.z]) : undefined;
      if (estimate !== undefined && estimate >= windowMs) {
        // A failed margin from OFF the mouth is a stance problem, not a design
        // problem: the hold/re-stage above lost to whatever moved the bot (the
        // tide-mill current) — take another attempt, which re-stages first. Only
        // a failed margin from ON the compiler-pinned mouth is terminal: DW0378
        // proves every shipped window admits its designed crossing, so that
        // firing means the artifact disagrees with the world, and entering would
        // gamble the bot's life on a proof that no longer applies.
        if (staging && !nearCell(gate.feetCell(), staging)) {
          lastErr = new Error(
            `drifted off the staging mouth [${staging.join(", ")}] to ` +
              `[${from!.join(", ")}] during the window wait — re-staging`,
          );
          process.stderr.write(
            `[timed-gate] attempt ${attempt}: ${(lastErr as Error).message}\n`,
          );
          continue;
        }
        throw new Error(
          `${glabel}: crossing estimate ${(estimate / 1_000).toFixed(1)}s from ` +
            `[${from!.join(", ")}] does not fit the ${(windowMs / 1_000).toFixed(1)}s open ` +
            `window of crushing ${describeGates(staged.filter((g) => g.crush))} — refusing ` +
            `to enter a crush gate without full margin (DW0378 proves the designed ` +
            `crossing fits from the pinned mouth, where the bot is staged)`,
        );
      }
    }
    const alabel = `${glabel} gate attempt ${attempt}`;
    try {
      // A crush entry crosses RAW first (zero pathfinder latency — the flood
      // through the freshly opened plane outruns a pathfinder start), then the
      // ordinary goto merely verifies/finishes the arrival from outside the fill.
      if (lethal && gate.dash && staging && crushWindowMs !== undefined) {
        const dashBudget = crushWindowMs - (now() - openedAt);
        const cleared = await gate.dash(staged, staging, [spec.x, spec.y, spec.z], dashBudget);
        if (!cleared) {
          throw new Error(
            `the raw crossing dash did not clear the gate within its ` +
              `${(Math.max(0, dashBudget) / 1_000).toFixed(1)}s window`,
          );
        }
      }
      await goto(spec, alabel);
      return "crossed";
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      if (done()) return "settled";
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
    // the ordinary stall escalation (pathfind → look-and-walk burst →
    // re-path), reused verbatim — plain movement a human player makes, still
    // bounded, and it still has to physically get through an open gate.
    //
    // Crush addendum (rule 2): the escalation is only run while the REMAINING
    // window still fits the crossing — bursting into a crush gate as it closes is
    // exactly the death this function exists to prevent. Out of margin ⇒ take the
    // next fresh window instead (the loop's next iteration).
    if (proven) {
      if (lethal) {
        const from = gate.feetCell() ?? ([proven.x, proven.y, proven.z] as Vec3Tuple);
        const windowMs = Math.min(...staged.filter((g) => g.crush).map((g) => openMs(g)));
        const remaining = windowMs - (now() - openedAt);
        if (crossingEstimateMs(from, [spec.x, spec.y, spec.z]) >= remaining) {
          process.stderr.write(
            `[timed-gate] attempt ${attempt}: remaining window ` +
              `${(Math.max(0, remaining) / 1_000).toFixed(1)}s is too short to escalate into a ` +
              `crush gate — waiting for the next fresh window\n`,
          );
          continue;
        }
      }
      try {
        await recoverAndRetry(spec, alabel, proven, goto, unstick);
        return "crossed";
      } catch (err) {
        if (err instanceof BotDeathError) throw err;
        if (done()) return "settled";
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
  // failure on it is the clock's doing. NOT for a crush-staged crossing (rule 3):
  // this recovery paths straight through the gate at an arbitrary clock position,
  // which on a crushing gate is the lethal blind entry itself.
  if (done()) return "settled";
  if (!lethal) {
    try {
      if (proven) {
        await recoverAndRetry(spec, glabel, proven, goto, unstick);
      } else {
        await goto(spec, glabel);
      }
      return "crossed";
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      if (done()) return "settled";
      lastErr = err;
    }
  }
  const detail = lastErr instanceof Error ? lastErr.message : String(lastErr);
  throw new Error(
    `${glabel}: still blocked after ${attempt} timed-gate crossing attempt(s) over ` +
      `${((now() - start) / 1_000).toFixed(1)}s — more than two full cycles of ` +
      `${describeGates(staged)}. The window is not the problem; this is a real ` +
      `navigation failure: ${detail}`,
  );
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

/**
 * A `/trigger …` a step sent, and every chat line the server answered it with.
 * The bot is opped, so vanilla's command feedback — the success
 * `Triggered [obj] (set value to n)` and every refusal — arrives on the same chat
 * stream the completion markers do; capturing it is what lets a timed-out step say
 * whether its trigger reached the delve at all.
 */
export interface TriggerEcho {
  readonly command: string;
  /** The scoreboard objective the command names, e.g. `dw.dlg_eurylochus`. */
  readonly objective: string;
  /** Server answers observed since the command was sent, in order. */
  readonly lines: string[];
}

/**
 * What the server said about a step's `/trigger`, as a clause appended to that
 * step's objective-timeout message. Diagnostics only — it decides nothing and
 * relaxes nothing; a step that times out still fails.
 *
 * Why it exists (and round 13's `requires_item` defect before it): a
 * swallowed trigger and an undelivered one produce the identical bare 30s timeout,
 * and telling them apart cost a full round of misattributed red runs each time. The
 * server answers every `/trigger` — `Triggered [obj] (set value to n)` on success, a
 * refusal otherwise — and the bot, opped, receives that answer on the same chat
 * stream the completion markers arrive on. So the harness repeats it back:
 *   - answered ⇒ the delve's own guard consumed the trigger without completing the
 *     objective. The classic cause is a REUSED world: a scoreboard that already
 *     carries the objective makes its `unless score … matches 1` guard a no-op, so
 *     nothing completes and nothing is broadcast (island round 13, and again here);
 *   - unanswered ⇒ the command never reached the delve at all, which is the
 *     harness's own problem, not the campaign's.
 *
 * The unanswered reading assumes the delve leaves `sendCommandFeedback` alone, which
 * every compiled delve does — `setup.mcfunction`'s gamerule block never touches it.
 * A campaign that suppressed feedback would silence the answer and read as
 * unanswered; if that ever becomes possible, this must be told, not left to infer.
 */
export function swallowedTriggerVerdict(echo: TriggerEcho | undefined): string {
  if (!echo) return "";
  if (echo.lines.length === 0) {
    return (
      `; the server never answered \`${echo.command}\` — the trigger did not reach the ` +
      `delve (refused or undelivered), so this is a harness/infrastructure failure, ` +
      `not a content one`
    );
  }
  return (
    `; the server ANSWERED \`${echo.command}\` with ${echo.lines.map((l) => `"${l}"`).join(", ")} ` +
    `— the trigger reached the delve and its own guard consumed it without completing ` +
    `the objective. The usual cause is a re-used world whose scoreboard already ` +
    `carries this objective (its \`unless score … matches 1\` guard then completes ` +
    `nothing): tear the stack down with \`validation/fresh-volumes.sh --project ` +
    `<compose-project>\` and re-run on a proven-clean world`
  );
}

/**
 * The scoreboard objective a `/trigger <objective> …` command names, or `undefined`
 * for anything that is not a trigger command.
 */
export function triggerObjective(command: string): string | undefined {
  return /^\/trigger\s+(\S+)/.exec(command.trim())?.[1];
}

/**
 * Whether a chat line is the server ANSWERING a `/trigger <objective>`. Two shapes,
 * because vanilla's success and failure messages are worded independently:
 *   - success names the objective (its display name defaults to its id):
 *     `Triggered [dw.dlg_eurylochus] (set value to 4)`;
 *   - the refusals do not name it, but all of them say "trigger":
 *     `You can't trigger this objective yet`, `This objective is not a trigger`.
 * Only ever consulted inside the window between the harness sending a trigger and
 * that step finishing, where the only such traffic is the answer to our own command.
 */
export function answersTrigger(message: string, objective: string): boolean {
  return message.includes(objective) || /trigger/i.test(message);
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
 * How far from its anchor cell an actor's unleashed body may be and still be
 * recognised as that actor. Generous but local: `unleash-actor` replaces
 * the puppet with a real-AI twin at the same cell, and the twin then MOVES — it
 * charges the bot — so a radius tight enough to be an identity check would lose
 * the fight it just started. Nothing else of that entity type exists nearby: the
 * delve world is sealed (`spawn_mobs false`), so every living body is
 * compiler-summoned.
 */
const ACTOR_MATCH_RADIUS = 24;

/** How long to wait for the unleashed twin to exist and reach entity tracking. */
const ACTOR_SETTLE_MS = 6_000;

/**
 * Budget for one unassisted actor attempt. Shorter than the wave `kill` budget on
 * purpose: nothing downstream waits on this fight, and a bot that has not killed
 * a single body in half a minute of swinging has already answered the only
 * question the floor gate asks.
 */
const ACTOR_FIGHT_TIMEOUT_MS = 45_000;
/**
 * How long (ms) the die-retry stage trades blows before taking its `mid-fight`
 * scripted death (spec-0023 §1). Short on purpose: the point is that the wave has
 * been ENGAGED — some mobs hurt, the fight's state dirty — when the death lands,
 * because that is the state a respawn has to restore. Winning the fight here would
 * defeat the trial.
 */
const MID_FIGHT_MS = 6_000;
// (how long the bot may swing at one body before giving up on it is no longer a
// constant here: it is `bodies[].give_up_swings` in the combat plan, per kind,
// out of the encounter's own arithmetic. See `giveUpBudgetFor`.)
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
 * Margin (ms) added on top of a path's exported `ending_tail_ticks` when sizing
 * the completion window: the compiler schedules the ending's finale
 * — the-wake fires `campaign-complete` 250t into its closing `sequence` — and
 * exports that tail on the terminal step; the window must outlive the tail plus
 * server slack. See {@link completionWindowMs}.
 */
const ENDING_TAIL_MARGIN_MS = 10_000;
/** Minecraft server ticks per second (the tick → wall-clock conversion). */
const TICKS_PER_SECOND = 20;

/**
 * How long (ms) `assertComplete` may wait for the completion marker: the default
 * settle window, widened — never narrowed — by the path's exported
 * scheduled-ending tail (`ending_tail_ticks` + margin). Exported for its unit
 * test; pure arithmetic, no bot state.
 */
export function completionWindowMs(endingTailTicks: number | undefined): number {
  const tailMs = ((endingTailTicks ?? 0) * 1000) / TICKS_PER_SECOND;
  return Math.max(SCORE_SETTLE_MS, tailMs + ENDING_TAIL_MARGIN_MS);
}
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
 * Physics-unstick: a SHORT forward tap per burst (~a cell, not a launch —
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
 * gap 8: a server-forced position jump of at least this many blocks
 * (horizontal) is treated as a cross-area teleport rather than knockback or a
 * within-area nudge. Areas sit ~256 blocks apart across void (an unambiguous jump);
 * in-area relocations (spawn, class teleport) stay well under this, so the threshold
 * cleanly separates a transport from ordinary forced moves. When one is observed the
 * pathfinder is reset, so a path computed in the OLD area cannot survive the jump and
 * strand the next step with a spurious "No path to the goal!".
 */
const TRANSPORT_JUMP_BLOCKS = 64;
/**
 * gap 8: after the jump lands, how long (ms) to wait for the destination
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
 * the instant the declared wave is standing. The probe asks the SERVER, by tag,
 * so what it settles on is the wave itself — client tracking range does not bound
 * the answer, and nothing standing nearby can enter it.
 */
const REENGAGE_SETTLE_MS = 6_000;

/**
 * How long one census may take to come back.
 *
 * A census is a `/function` call whose answer arrives on the chat channel within
 * a tick or two; this is the "the command was refused" deadline, not a settle.
 * The census returns `undefined` on expiry — never a zero, which would read as
 * "the wave is gone" and turn an unopped bot into a false `stranded` verdict.
 */
const CENSUS_TIMEOUT_MS = 3_000;

/**
 * How often the kill step asks the server whether the wave still stands, while
 * nothing else has prompted it to.
 *
 * The step's terminal condition is the census (`wave.ts`), so this is the
 * background cadence that condition runs on: a census is a `/function` round trip
 * plus a chat line per standing mob, which is cheap at this rate and would be
 * noise at the loop's own 250ms. It is a FLOOR, not a schedule — a guess that the
 * fight is over asks at once, and so does the bot's own tally reaching the wave's
 * declared size, which is exactly when the old code asked.
 */
const WAVE_CENSUS_POLL_MS = 2_000;

/** How many censuses' mob lines stay addressable. Only the newest is ever asked
 * for; the rest are kept so a late line cannot grow the map without bound. */
const CENSUS_HISTORY = 4;

/**
 * How long the post-respawn re-arm waits for the kept kit to arrive on the wire.
 *
 * A respawn keeps the kit — the compiler seals `gamerule keep_inventory true` in
 * every build — but the inventory is re-sent to the client a few ticks after the
 * spawn packet, so an immediate read sees an empty bag that is only empty yet.
 * The wait returns the instant an item shows up; only a bag still empty at the
 * deadline is a bag that lost its kit.
 */
const KIT_SETTLE_MS = 3_000;

/** How far from a rest step's anchor cell its `interaction` affordance may sit. */
const AFFORDANCE_RADIUS = 3;

/** Vanilla standing-player hitbox, 1.21.11 — the body the stance sweep has to fit
 * into a cell, and the reason a player can never share a column with an NPC. */
const PLAYER_HITBOX_WIDTH = 0.6;
const PLAYER_HITBOX_HEIGHT = 1.8;

/** Vanilla standing eye height, 1.21.11: where the entity-pick ray starts. */
const PLAYER_EYE_HEIGHT = 1.62;

/** Slack beyond interaction reach when collecting bodies that might occlude a
 * target: a wide body whose CENTRE is past reach can still put a shoulder in the
 * ray, so the search is generous and the ray does the deciding. */
const CROSSHAIR_SEARCH_MARGIN = 5;

/** The walk-goal radius of each interaction step — and therefore the set of
 * standing cells the crosshair sweep is entitled to try. One constant per step so
 * the goal the bot walks to and the stances it is judged over can never drift. */
const TALK_RANGE = 3;
const INTERACT_RANGE = 3;
const REST_RANGE = 2;

/** Grace between the bonfire click and the trigger command: the opener runs as an
 * advancement reward, so `dw.rest` is enabled a tick or two after the click. */
const REST_OPEN_SETTLE_MS = 500;
/** Recent chat lines retained for death-cause diagnosis. */
const CHAT_BUFFER = 16;

// --- the death loop ---------------------------------------------
/**
 * How long to wait, after stepping into a declared lethal volume, for the player
 * to actually die. A volume's driver runs in the campaign `tick`, so the kill is
 * one tick away; this is a generous ceiling on "one tick", not a guess at how long
 * dying takes.
 */
const LETHAL_DEATH_TIMEOUT_MS = 10_000;
/**
 * How long to let the ledger settle after the respawn before reading it.
 *
 * The ORDER is guaranteed by the engine, not by this number: the death edge fires
 * `on_death` on the corpse (`if Health:0.0f`) and the re-seat on the living player
 * (`unless Health:0.0f`), so by the time a respawn is observed the forfeit has
 * already run. The poll below therefore stops the instant the ledger reaches the
 * value the campaign promised, and this ceiling only bounds the case where it
 * never does — which is the finding, not a flake.
 */
const LEDGER_SETTLE_MS = 5_000;
const LEDGER_POLL_MS = 100;
/** How long to wait for a scoreboard objective to start reporting after tracking it. */
const SCORE_TRACK_TIMEOUT_MS = 5_000;
/** How long to wait for the collected stake's hardware to be retired. */
const MARKER_RETIRE_TIMEOUT_MS = 5_000;
/** How far from the table's anchor the marker's own hardware is looked for. */
const MARKER_SEARCH_RADIUS = 4;
/**
 * How close the glowing display must stand to the interaction for the two to be
 * ONE stake. `stk_fill_<s>` summons both at the same position in one function, so
 * this is a tolerance on floating point and on a client's rounding of it, never a
 * search radius: at four blocks any display in the neighbourhood vouched for any
 * interaction in it.
 */
const MARKER_PAIR_RADIUS = 0.5;
/**
 * The pathfinder cost that makes a cell impassable. The library treats a step
 * whose total cost exceeds 100 as no move at all (`movements.js`: `if (cost > 100)
 * return`), so anything above it is a refusal rather than a preference.
 */
const LETHAL_STEP_COST = 1_000;
/**
 * How long the bot leaves its own corpse on the death screen before taking the
 * respawn — one human beat (20 server ticks).
 *
 * Not a tuning knob and not a wait for a race to settle: it is the difference
 * between a player and a library. mineflayer answers the death packet in the same
 * event-loop turn, so a default bot is alive again on the next tick and the
 * engine's whole corpse-side death edge is unobservable — the branch is guarded by
 * `if data entity @s {Health:0.0f}`, and it is where `on_death` fires. A vanilla
 * player has to click Respawn, so a corpse always exists for many ticks.
 *
 * Kept small enough that the 15 s respawn budget is untouched, and comfortably
 * inside the 59-tick post-respawn invulnerability window that spec-0032 records —
 * nothing here forces a second death.
 */
const DEATH_SCREEN_HOLD_MS = 1_000;
/**
 * How often (ms) {@link MineflayerExecutor.awaitEntitySettle} polls the non-player
 * entity count while waiting for the tracker to stop changing shape after spawn.
 */
const ENTITY_SETTLE_POLL_MS = 200;
/**
 * Hard ceiling (ms) on the post-spawn entity-settle wait (2026-08-06 island
 * triage). World-persisted entities were observed populating by t+4s; this
 * leaves comfortable margin without letting a build that never spawns anything
 * near the bot hang the run — the wait always gives up and proceeds, and the
 * existing "not tracked" crosshair warning stays honest about whatever state the
 * tracker is actually in when a step reads it.
 */
const ENTITY_SETTLE_TIMEOUT_MS = 8_000;
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
   * The one owner of the pathfinder's goal. Every trip is issued and collected
   * through it, so a hop this executor walks away from (a death, a timeout, a
   * stalker winning the race) still has its rejection read — see navigation.ts.
   */
  private readonly nav = new NavigationOwner(() => this.bot?.pathfinder);
  /**
   * Which labelled stage the executor is inside RIGHT NOW. Read by the crash
   * reporter, which has no other way to know: the ladder's stages are assembled
   * from the outside once the run is over, and a process that dies mid-run never
   * gets there. Only the boundaries that a crash could plausibly sit inside are
   * marked — a crash anywhere else is on the critical path by construction.
   */
  private stageNow: StageName = "critical-path";
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
   * One exact line the run is currently watching for, and whether it has
   * arrived. Armed around a single act (walking into a lethal volume) rather than
   * mined out of {@link recentChat}, because that ring holds sixteen lines and a
   * death broadcasts several — a wording assertion that can be pushed out of a ring
   * is an intermittent test, which is an under-specified one.
   */
  private wordWatch: { readonly needle: string; seen: boolean } | undefined;
  /**
   * The scoreboard ledgers this run is observing, `objective → entry →
   * value`, fed straight off the wire.
   *
   * **Why the raw packet and not `bot.scoreboards`.** mineflayer 4.37's scoreboard
   * plugin gates every score update on `packet.action === 0`, and 1.21.11 has no
   * `action` field on `scoreboard_score` at all (it was split out into
   * `reset_score`), so its model never updates on the pinned version. The packet
   * itself decodes perfectly — `{itemName, scoreName, value}` — so the harness
   * reads it directly. This is observation, not game logic: no delve score is ever
   * written from here.
   */
  private readonly scores = new Map<string, Map<string, number>>();
  /** The objective currently occupying the harness's display slot, if any. */
  private trackedObjective: string | undefined;
  /**
   * The lethal volumes the build declares, as impassable boxes for the
   * PATHFINDER. The compiler already treats them as impassable in every route
   * proof; without the same fact here the bot walks its way back from a death
   * straight through the hazard it just died in, and a run intermittently dies
   * twice for reasons no content author could reproduce.
   */
  private lethalBoxes: readonly Box[] = [];
  /** Suspended for exactly one walk: the deliberate step INTO a volume. */
  private lethalExclusionSuspended = false;
  /** Every walk into a lethal volume this run made, and what it observed. */
  private readonly lethalTrials: LethalTrial[] = [];
  /** Why the death-loop stage did not run, when it did not. */
  private deathLoopSkip: string | undefined;
  /**
   * The build's death contract — what the campaign PROMISES a death does.
   * Absent → a run in which nothing about dying is asserted at all (which is
   * the gap this stage exists to close).
   */
  private deathPlan: DeathPlan | undefined;
  /** The `/trigger` the current step sent, and the server's answer to it.
   * Replaced by each new trigger; see {@link swallowedTriggerVerdict}. */
  private trigger: TriggerEcho | undefined;
  /**
   * gap 8: the bot position captured at the previous server-forced move
   * (`forcedMove`). A forced move whose horizontal delta from this reaches
   * {@link TRANSPORT_JUMP_BLOCKS} is a cross-area teleport; used to reset the
   * pathfinder so a path computed in the old area cannot survive the jump.
   */
  private lastForcedPos: { x: number; y: number; z: number } | undefined;
  /** Grace (ms) added onto a cutscene's declared length before giving up. */
  private readonly cutsceneGraceMs: number;
  /** Hard ceiling (ms) on the post-spawn entity-settle wait. Overridable
   * (`DELVEWRIGHT_ENTITY_SETTLE_TIMEOUT_MS`) so a test can shorten the give-up
   * path without waiting out the production default. */
  private readonly entitySettleTimeoutMs: number;
  /**
   * The compiler's proven per-leg critical-path waypoints (keyed by
   * destination anchor). When a walked step's target has a leg here, `walkTo`
   * replays it as successive nearby goals so each mineflayer A* solve is trivial —
   * instead of one distant goal that strands the bot on a large open winding cave.
   * Absent → the original single distant-goal behavior (fallback). Compiler-proven
   * navigation data, not a route the harness computes.
   */
  private waypoints: Waypoints | undefined;
  /**
   * The delve's own statement of which entity kinds are never a combat target,
   * off `critical-path.json` (format 4). There is deliberately no default: the
   * only fallback available is a set of entity names living in this file, which
   * is the thing the field exists to delete. The path parser refuses a document
   * without it, and {@link requireNonCombatants} refuses a run that never wired
   * it through.
   */
  private nonCombatants: ReadonlySet<string> | undefined;
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
   * aborted run still carries the death it took. */
  private readonly trials: DeathTrialRecord[] = [];
  /** Waves the die-retry stage entered, whether or not it finished with them.
   * Engagement without records is the silence the run report must not keep. */
  private readonly dieRetryEngaged = new Set<string>();
  /** Every `rest` step the PATH declares, and which of them the bot performed —
   * the die-retry precondition reads both. Declared up front
   * rather than accumulated as they run, so "the route passed this fire without
   * resting" is a statement the check can actually make. */
  private restSteps: readonly PerformedRest[] = [];
  private readonly restedBonfires = new Set<number>();
  /** Encounters whose scripted deaths were SKIPPED because the checkpoint the
   * stage would measure against was never armed — the RUN's own gap, and red. */
  private readonly preconditionFindings: string[] = [];
  /** Encounters whose scripted deaths were skipped because the campaign fires NO
   * checkpoint before them — a content fact, reported and not graded. */
  private readonly preconditionAdvisories: string[] = [];
  private readonly preconditionWaves = new Set<string>();
  /** The newest census summary the chat channel has delivered, and the mob lines
   * that closed each census, keyed by the server's own sequence number.
   * `censusSeq` is how a fresh answer is told from a stale one without the
   * harness ever writing a delve score to ask its question. */
  private censusSummary: CensusSummary | undefined;
  private censusSeq = 0;
  private readonly censusMobs = new Map<number, CensusMob[]>();
  /** How far `kill()` got with each encounter — the reading key for an empty
   * `assist_windows` array (spec-0023 takes no assist while deliberately dying,
   * nor on a billed encounter's honest first attempt). */
  private readonly encounterPhases = new Map<string, EncounterPhase>();
  /** Inverted floor gate findings: billed fights the unassisted bot beat cold. */
  private readonly floorFindings: string[] = [];
  /**
   * Who felled each wave's bodies, as its last census answered. The floor gate's
   * verdict reads it, and so does the run report: an encounter cleared because a
   * lethal volume ate two thirds of its cohort is not an encounter the bot beat,
   * and before this nothing anywhere could tell the two apart.
   */
  private readonly waveAttributions = new Map<string, FightAttribution>();
  /** Every body that outlived its kind's melee budget, with the arithmetic
   * that budgeted it. Recorded rather than absorbed: the six-second timer
   * this replaced blacklisted such a body and said nothing at all. */
  private readonly unkillableBodies: UnkillableBody[] = [];
  /** Every NAMED entity death this run observed (`entityDead` on a body carrying a
   * custom name — an actor). Raw and unclassified; the entrypoint classifies each
   * against {@link classifyDeathDepth} (see teardown.ts) before it reaches the run
   * report — the 2026-08-06 island triage found that a scripted `despawn-actor`
   * vanish broadcasts the same "<name> died" line a real combat loss does. */
  private readonly namedEntityDeathLog: NamedEntityDeath[] = [];
  /** Actor fights this run attempted — one entry per engagement, won or lost. */
  private readonly actorTrials: ActorTrial[] = [];
  /** Actors already engaged, so an objective marker re-broadcast cannot re-fight one. */
  private readonly actorsEngaged = new Set<string>();
  /** Whether the actor floor gate runs at all (`DELVEWRIGHT_ACTOR_FLOOR=0` skips). */
  private actorFloorGate = true;
  /** Objectives the compiled path proves — decides which actor fights are reachable. */
  private pathObjectives: ReadonlySet<string> = new Set();
  /** How many items the bot was carrying the last time it was known to be alive
   * and kitted. The baseline `keep_inventory` is judged against after a death. */
  private itemsBeforeDeath = 0;
  /**
   * How many walked legs have been consumed. Legs are matched in lockstep
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
    const settleRaw = env["DELVEWRIGHT_ENTITY_SETTLE_TIMEOUT_MS"];
    const settleParsed = settleRaw === undefined ? NaN : Number.parseInt(settleRaw, 10);
    this.entitySettleTimeoutMs =
      Number.isInteger(settleParsed) && settleParsed >= 0 ? settleParsed : ENTITY_SETTLE_TIMEOUT_MS;
  }

  /**
   * Supply the compiler's proven critical-path waypoints. Optional —
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
      // mineflayer's auto-respawn answers the death packet in the SAME
      // event-loop turn it arrives in, so the bot is alive again on the very next
      // server tick. No human is: the death screen requires a click, and the
      // engine's death edge is specified ON THE CORPSE (spec-0031: the
      // `deathCount` edge is armed pre-respawn, and `cp_respawn_check` reads it
      // with `if data entity @s {Health:0.0f}`). A corpse that never exists for a
      // whole tick makes the whole `on_death` branch unreachable — and a validator
      // that cannot observe a mechanism because of its own client library is a
      // validator that will report the mechanism green forever.
      //
      // So the respawn is taken MANUALLY, one human beat later ({@link
      // DEATH_SCREEN_HOLD_MS}). Nothing else changes: every existing wait is
      // counted off `spawnSeq`, which still rises exactly once per respawn.
      respawn: false,
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
    // Entity-tracker settle race (2026-08-06 island triage): `bot.entities` is empty
    // for a few seconds after spawn while world-persisted entities' packets are still
    // arriving. Waiting here, once, before the run does anything with `bot.entities`
    // (the crosshair sweep foremost) is cheaper and more honest than teaching every
    // caller of `hitboxesNear` to guess whether an empty read means "nothing there"
    // or "not yet told" — see entity-settle.ts.
    await this.awaitEntitySettle();
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
      this.observeCensus(message);
      if (this.wordWatch && message.includes(this.wordWatch.needle)) {
        this.wordWatch.seen = true;
      }
      if (this.trigger && answersTrigger(message, this.trigger.objective)) {
        this.trigger.lines.push(message);
      }
      this.recentChat.push(message);
      if (this.recentChat.length > CHAT_BUFFER) {
        this.recentChat.shift();
      }
    });
    // The scoreboard ledgers, straight off the wire. Both packets are
    // read because 1.21.11 split "set a score" and "clear a score" into two — a
    // reader that watches only the first reports a cleared purse as its last known
    // value, which is the one direction a currency assertion must never drift in.
    this.installScoreObserver(bot);
    bot.on("death", () => this.onDeath());
    // Scripted-teardown death classification (2026-08-06 island triage): `entityDead`
    // fires on the LivingEntity death status packet, while the entity's last known
    // position is still readable — unlike `entityGone`, which also fires for an
    // ordinary despawn (out of render distance, dimension change) and says nothing
    // about a death. Only named bodies are actors the compiler ever tears down or a
    // story fight ever names; an unnamed mob's death is not this run report's concern.
    bot.on("entityDead", (entity: Entity) => this.onNamedEntityDeath(entity));
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
    // gap 8: mineflayer applies a server position packet to
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
   * can actually use — the synchronous half of {@link NavigationOwner}, which is
   * what the death handler and the forced-move handler need.
   *
   * The stop/`setGoal(null)` pairing, and why it is a pairing, live on
   * {@link NavigationOwner.stopNow}. What matters here: the cancelled trip stays
   * registered with the owner, so its `GoalChanged`/`PathStopped` rejection is still
   * observed when it arrives, and the next hop waits for it before setting a goal.
   */
  private stopPathfinding(): void {
    this.nav.stopNow();
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
      if (!entity?.position || !isWaveMob(entity, bot.entity, this.requireNonCombatants()))
        continue;
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
      // Observed, not fired and forgotten: `currentStalker` reads live bot state,
      // and a throw from a detached poll is an unhandled rejection — fatal under
      // Node's default, for a watch whose whole job is advisory.
      poll().catch(() => {
        // A watch that cannot read the world simply never fires; the walk it is
        // racing still finishes or fails on its own terms.
      });
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
    await this.awaitObjectiveMarker(objectiveId, label);
    // spec-0023's floor gate, on the OTHER shape an elite takes: an
    // actor fight has no `kill` step, so the only moment the harness can know it
    // starts is the completion of the objective that unleashes it. Runs here, once
    // per actor, after the objective it hangs off is proven — never before.
    await this.actorFloorGateAfter(objectiveId);
  }

  /** The completion wait itself — see {@link requireObjective}. */
  private async awaitObjectiveMarker(objectiveId: string, label: string): Promise<void> {
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
        `completed so far: ${seen.join(", ") || "none"}${swallowedTriggerVerdict(this.trigger)}`,
    );
  }

  /**
   * Send a step's `/trigger …` command and start listening for the server's own
   * answer to it (see {@link TriggerEcho}). Every trigger-driven step goes through
   * here, so a step that times out can always say whether its trigger REACHED the
   * delve — the one fact that separates a content/state failure from a harness one.
   */
  private chatTrigger(command: string): void {
    this.armTrigger(command);
    this.requireBot().chat(command);
  }

  /**
   * Start listening for the server's answer to `command` without sending it — for
   * the steps whose chat is issued by a helper (`interact` goes through
   * `presentAndTrigger`, which must equip first and chat last).
   */
  private armTrigger(command: string): void {
    const objective = triggerObjective(command);
    this.trigger = objective === undefined ? undefined : { command, objective, lines: [] };
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
    // A trigger echo belongs to the step that sent it. Dropping it here keeps a
    // later step's timeout from quoting the previous step's `/trigger` as if it
    // were its own — a diagnostic that names the wrong command is worse than none.
    this.trigger = undefined;
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
      // EXACT, never rounded. `Math.round` here produced a triple that is neither
      // a position nor the cell the body was in, and the death-loop stage read it
      // as a cell: a kill at `z = 4.6` (cell 4, inside the volume) rounded to 5
      // and was reported as a death outside the box that killed it. Whoever wants
      // a cell floors; whoever wants "the volume's selector matched this body"
      // asks `bodyInVolume`.
      position = [p.x, p.y, p.z];
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
    // The respawn a player takes, not the one a library takes for them (see the
    // `respawn: false` note in `connect`). Held one human beat so the corpse
    // exists for at least one server tick — which is where the engine's death
    // edge lives.
    const takeRespawn = (): void => {
      try {
        bot?.respawn();
      } catch {
        // A failed respawn is not lost: `recoverFromDeath` bounds its own wait and
        // reports a missing respawn loudly.
      }
    };
    setTimeout(takeRespawn, DEATH_SCREEN_HOLD_MS).unref?.();
  }

  /**
   * Record a named entity's death (raw — not yet classified scripted-teardown vs
   * combat; see teardown.ts). Every other body dying near the bot every fight is
   * silent about here on purpose: only a NAMED body is an actor the compiler's
   * `despawn-actor` can tear down or a tiered fight can lose, and the island's
   * report-legibility gap was specifically about those.
   */
  private onNamedEntityDeath(entity: Entity | undefined): void {
    if (!entity || entity.id === this.bot?.entity?.id) return; // the bot's own death is onDeath's
    const label = displayNameOf(entity);
    if (label === undefined || label === "") return;
    const p = entity.position;
    if (!p) return;
    this.namedEntityDeathLog.push({
      name: label,
      entityId: entity.id,
      position: [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)],
    });
  }

  /**
   * Race the bot dying against the operation `start` returns: resolves/rejects with
   * that operation, but rejects with the {@link BotDeathError} the instant a death is
   * observed (the underlying op keeps running but the pathfinder is already stopped
   * in {@link onDeath}). Used to abort the ~60s `pathfinder.goto` wait the moment the
   * bot dies.
   *
   * `start` is a THUNK, not a promise, and that is the whole of it: when a death is
   * already latched this returns without ever calling it, so there is no operation
   * to leave unobserved. Taking a promise instead meant the caller had already
   * built one by the time the early return fired — and a `pathfinder.goto` built and
   * then dropped rejects with `GoalChanged` the next time ANY goal is set, with no
   * handler attached, which is a fatal unhandled rejection under Node's default. A
   * boss that killed the bot mid-trade killed the whole run that way, three seconds
   * after the die-retry stage set its re-approach goal.
   */
  private raceDeath<T>(start: () => Promise<T>): Promise<T> {
    if (this.death) return Promise.reject(this.death);
    const op = start();
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
  /**
   * The labelled stage the executor is inside right now — what a crash report has
   * to say to stop a harness fault reading as a content verdict on whichever stage
   * happened to be next.
   */
  currentStage(): StageName {
    return this.stageNow;
  }

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
    // per encounter, straight out of the run budget (observed live on
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

  // -------------------------------------------------------------------------
  // The death loop: a real player dies, and every consequence the
  // engine promised is asserted from the observation.
  // -------------------------------------------------------------------------

  /**
   * Adopt the build's death contract. Also hands the declared lethal volumes to
   * the navigator, which has to agree with the compiler that they are impassable.
   */
  useDeathPlan(plan: DeathPlan): void {
    this.deathPlan = plan;
    this.lethalBoxes = plan.volumes.map((v) => v.region);
  }

  /** Every walk into a lethal volume this run made, and what it observed. */
  deathLoopTrials(): readonly LethalTrial[] {
    return this.lethalTrials;
  }

  /** Why the stage did not run, when it did not. `undefined` means it ran. */
  deathLoopSkipReason(): string | undefined {
    return this.deathLoopSkip;
  }

  /**
   * Watch the ledgers off the wire. See {@link scores} for why the raw packet and
   * not mineflayer's own scoreboard model.
   */
  private installScoreObserver(bot: Bot): void {
    // The unit tests attach a stub bot that models only the high-level API, so a
    // missing raw client is "there is no wire here", not a fault. Absent → no
    // ledger is ever observed, and every currency assertion reports that it could
    // not read rather than passing.
    const client = bot._client as Bot["_client"] | undefined;
    if (typeof client?.on !== "function") return;
    client.on("scoreboard_score", (packet: unknown) => {
      if (typeof packet !== "object" || packet === null) return;
      const p = packet as { itemName?: unknown; scoreName?: unknown; value?: unknown };
      if (typeof p.itemName !== "string" || typeof p.scoreName !== "string") return;
      if (typeof p.value !== "number") return;
      let board = this.scores.get(p.scoreName);
      if (!board) {
        board = new Map<string, number>();
        this.scores.set(p.scoreName, board);
      }
      board.set(p.itemName, p.value);
    });
    client.on("reset_score", (packet: unknown) => {
      if (typeof packet !== "object" || packet === null) return;
      const p = packet as { entity_name?: unknown; objective_name?: unknown };
      if (typeof p.entity_name !== "string") return;
      if (typeof p.objective_name === "string") {
        this.scores.get(p.objective_name)?.delete(p.entity_name);
        return;
      }
      for (const board of this.scores.values()) board.delete(p.entity_name);
    });
  }

  /**
   * Put `objective` where the server will report it, and wait until it does.
   *
   * A vanilla server only tracks — and therefore only broadcasts — an objective
   * that occupies a display slot, so reading one means asking for it. This is a
   * HARNESS action on the world, of exactly the class spec-0023 already sanctions
   * for `/damage @s` and `/effect give`: it is applied and removed by the harness,
   * it is named in the run report, and no delve content can reach it. The shipped
   * image is untouched — the bot is opped by a compose environment variable.
   *
   * Returns false when the objective never starts reporting: the bot is not opped,
   * or the objective does not exist. Either is a finding, never a silent pass.
   */
  private async trackScore(objective: string): Promise<boolean> {
    const bot = this.requireBot();
    if (this.trackedObjective === objective && this.scores.has(objective)) return true;
    // Swapping the slot STOPS the server tracking the previous objective, so its
    // cached values freeze at whatever they last were — and a frozen ledger read as
    // a live one is the exact shape of a currency assertion that passes over a
    // forfeit that never happened. Drop it rather than keep it.
    if (this.trackedObjective !== undefined) this.scores.delete(this.trackedObjective);
    bot.chat(`/scoreboard objectives setdisplay sidebar ${objective}`);
    this.trackedObjective = objective;
    const ok = await this.waitFor(
      () => this.scores.get(objective) !== undefined,
      SCORE_TRACK_TIMEOUT_MS,
      LEDGER_POLL_MS,
    );
    if (!ok) {
      process.stderr.write(
        `[death-loop] the server never reported objective '${objective}' after it was put on ` +
          `the sidebar. Either the bot is not opped (compose sets DELVE_OPS_OFFLINE to the bot's ` +
          `name — check it matches DELVEWRIGHT_BOT_USERNAME) or the delve declares no such ` +
          `ledger. The currency assertions cannot be made\n`,
      );
    }
    return ok;
  }

  /** Release the harness's display slot. Idempotent; best effort. */
  private clearScoreDisplay(): void {
    if (this.trackedObjective === undefined) return;
    try {
      this.requireBot().chat("/scoreboard objectives setdisplay sidebar");
    } catch {
      // teardown must never mask the run's own verdict
    }
    this.scores.delete(this.trackedObjective);
    this.trackedObjective = undefined;
  }

  /** The bot's own value in a tracked ledger, or `undefined` if it has none. */
  private myScore(objective: string): number | undefined {
    return this.scores.get(objective)?.get(this.config.username);
  }

  /**
   * Wait until a tracked ledger reads `want`, then return whatever it reads.
   *
   * It stops early on the promised value and otherwise runs the ceiling out, so a
   * correct engine is fast and a wrong one still yields its real number for the
   * failure message. This is not "waiting for green": the value returned is the
   * observation, and the caller asserts against it either way.
   */
  private async settledScore(objective: string, want: number): Promise<number | undefined> {
    await this.waitFor(() => this.myScore(objective) === want, LEDGER_SETTLE_MS, LEDGER_POLL_MS);
    return this.myScore(objective);
  }

  /** Make every declared lethal volume impassable to the pathfinder. */
  private applyLethalExclusion(movements: InstanceType<typeof Movements>): void {
    if (this.lethalExclusionSuspended || this.lethalBoxes.length === 0) return;
    const boxes = this.lethalBoxes;
    movements.exclusionAreasStep.push((block): number => {
      const p = block.position;
      const cell: Vec3Tuple = [p.x, p.y, p.z];
      return boxes.some((b) => inBox(cell, b)) ? LETHAL_STEP_COST : 0;
    });
  }

  /**
   * **The stage.** Walk into every declared lethal volume, die there, and assert
   * every consequence the campaign promised from what was OBSERVED.
   *
   * One trial per declared volume, and the loop is self-restoring: collecting the
   * stake puts the purse back, so the next volume's trial starts from a full
   * ledger rather than from a zero the previous death left.
   */
  async runDeathLoop(): Promise<void> {
    this.stageNow = "death-loop";
    const plan = this.deathPlan;
    if (plan === undefined) {
      this.deathLoopSkip = "this build ships no validation/death-plan.json";
      return;
    }
    if (plan.binding.unbound) {
      this.deathLoopSkip = plan.binding.reason ?? "the build's death plan binds to nothing";
      return;
    }
    try {
      for (const volume of plan.volumes) {
        await this.lethalTrial(plan, volume);
      }
    } finally {
      this.clearScoreDisplay();
    }
  }

  /** One volume: approach, step in, die, and assert the aftermath. */
  private async lethalTrial(plan: DeathPlan, volume: DeathPlan["volumes"][number]): Promise<void> {
    const bot = this.requireBot();
    // A trial NEVER opens over an unrecovered death. `stepInto` rethrows the death
    // latch on its first line, so a bot still lying dead from the previous trial's
    // walk back never takes a step, never dies again, and the trial then reports
    // that the bot stood in this volume and survived it — a verdict about a delve,
    // produced by the harness leaving its own bot on the death screen. The
    // previous trial's `deathPos` would also be read as this one's.
    if (this.death !== undefined) {
      process.stderr.write(
        `[death-loop] ${volume.id}: the bot was still dead when this trial opened — ` +
          `recovering before the approach, because a corpse cannot walk into anything\n`,
      );
      await this.recoverFromDeath();
    }
    const here = this.feetCell() ?? [0, 0, 0];
    const entryCell = entryCellOf(volume.region, here, (c) => this.bodyCanOccupy(c));
    // Which stake this death is supposed to leave. `on_death`'s own declaration
    // decides — never "the first one declared" — so a campaign whose death drops
    // one of three stakes is asserted against the one it named.
    const stake: StakeRule | undefined = plan.stakes.find((s) => plan.dropsStake.includes(s.id));
    if (entryCell === undefined) {
      // Every cell of the declared box is filled by a block. That is a finding
      // about the campaign — nothing can ever die in this volume — and it is
      // stated as one rather than by driving at a wall for ten seconds.
      const trial = openLethalTrial(volume, volume.region.lo, stake);
      trial.abandoned =
        `no cell of the declared volume [${volume.region.lo.join(", ")}]..` +
        `[${volume.region.hi.join(", ")}] can hold a body: every one of them is filled by a ` +
        `block, so nothing can ever be inside this volume for it to kill`;
      this.lethalTrials.push(trial);
      return;
    }
    const trial = openLethalTrial(volume, entryCell, stake);
    this.lethalTrials.push(trial);
    // The near lip: the cell the placement table already proved is the reachable
    // point nearest this volume. Nothing new is computed — it is the anchor a
    // death here would leave its stake at, which is the same question as "where
    // does a player stand next to this".
    const lip = plan.rows.find((r) => plan.regions[r.region]?.volume === volume.id)?.anchor;
    process.stderr.write(
      `[death-loop] ${volume.id}: standing at [${here.join(", ")}]; walking into ` +
        `[${entryCell.join(", ")}] to die there via the near lip ` +
        `${lip ? `[${lip.join(", ")}]` : "(none — the build declares no placement row)"}` +
        `${stake ? `; expecting stake ${stake.id} on ledger ${stake.currency.objective}` : ""}\n`,
    );

    // --- the ledger before -------------------------------------------------
    if (stake) {
      if (await this.trackScore(stake.currency.objective)) {
        trial.balanceBefore = this.myScore(stake.currency.objective);
      }
      if (trial.balanceBefore !== undefined) {
        trial.expectedForfeit = expectedForfeit(stake.forfeit, trial.balanceBefore);
      }
    }

    // --- the walk toward the volume ----------------------------------------
    // Armed BEFORE the approach, not between the approach and the step in. The
    // observation is "the player entered the box and died", and which of the two
    // legs delivered them there is the harness's business, not the delve's: the
    // approach ends one block from a hazard, and a pathfinder that overshoots by a
    // block has still produced exactly the event under test. Attributing that to
    // "the lip could not be reached" is how a real, correct death got reported as
    // an infrastructure fault on this stage's first live run.
    //
    // The guard that keeps it honest is the POSITION check below: a death anywhere
    // outside the declared box is not this volume's death and is not credited.
    this.wordWatch = { needle: volume.message, seen: false };
    const deathsBefore = this.deathSeq;
    let navFault: string | undefined;
    if (lip) {
      try {
        await this.walkTo(lip, 1, `death-loop approach to ${volume.id}`);
      } catch (err) {
        if (!(err instanceof BotDeathError)) {
          navFault =
            `the near lip [${lip.join(", ")}] could not be reached: ` +
            `${err instanceof Error ? err.message : String(err)}`;
        }
      }
    }
    if (this.bodyInside(volume.region)) trial.enteredVolume = true;
    // The one leg of the whole run that is ALLOWED into the hazard — skipped when
    // the approach already delivered the death.
    if (navFault === undefined && this.deathSeq === deathsBefore) {
      this.lethalExclusionSuspended = true;
      try {
        await this.stepInto(volume.region, entryCell, trial);
      } catch (err) {
        if (!(err instanceof BotDeathError)) {
          navFault =
            `the walk into [${entryCell.join(", ")}] failed for a reason that is not a death: ` +
            `${err instanceof Error ? err.message : String(err)}`;
        }
      } finally {
        this.lethalExclusionSuspended = false;
      }
    }

    // --- the death ---------------------------------------------------------
    const observed =
      navFault !== undefined
        ? false
        : await this.waitFor(
            () => this.deathSeq > deathsBefore,
            LETHAL_DEATH_TIMEOUT_MS,
            LEDGER_POLL_MS,
          ).catch((err: unknown) => {
            if (err instanceof BotDeathError) return true;
            throw err;
          });
    trial.deathPos = this.death?.position ? [...this.death.position] : undefined;
    trial.wordingSeen = this.wordWatch?.seen === true;
    this.wordWatch = undefined;
    // Credited only when the player died INSIDE the declared box. A death on the
    // way there is a run fault, not this volume's kill, and crediting it would let
    // any lethal accident anywhere pass as a proof that this box works.
    const inside =
      trial.deathPos !== undefined && bodyInVolume(trial.deathPos, volume.region);
    // A death inside the box is itself an observation that the body was inside it.
    if (observed && inside) trial.enteredVolume = true;
    trial.died = observed && inside;
    if (navFault !== undefined) {
      trial.abandoned = navFault;
      return;
    }
    if (observed && !inside) {
      trial.abandoned =
        `the bot died at ${formatDeathPos(trial.deathPos)}, which is OUTSIDE the reach of ` +
        `the declared volume [${volume.region.lo.join(", ")}]..[${volume.region.hi.join(", ")}] ` +
        `— a body there is not one this volume's own selector can match, so that death is ` +
        `not this volume's kill and is not credited as one`;
      return;
    }
    if (!trial.died) {
      // Nothing downstream is meaningful, and every field stays at its honest
      // default so the report cannot read as if it had checked them.
      process.stderr.write(
        trial.enteredVolume
          ? `[death-loop] ${volume.id}: the bot is standing in the volume and is still alive\n`
          : `[death-loop] ${volume.id}: the bot never got its feet inside the volume, so the ` +
            `volume was not exercised — this says nothing about whether it kills\n`,
      );
      return;
    }
    process.stderr.write(
      `[death-loop] ${volume.id}: died at ${formatDeathPos(trial.deathPos)}` +
        `; the volume's own line ${trial.wordingSeen ? "reached" : "did NOT reach"} the player\n`,
    );

    // --- the respawn -------------------------------------------------------
    await this.recoverFromDeath();
    const p = bot.entity?.position;
    trial.respawnPos = p ? [p.x, p.y, p.z] : undefined;
    const seat =
      trial.respawnPos === undefined ? undefined : seatAtRespawn(plan.seats, trial.respawnPos);
    trial.respawnSeat = seat === undefined ? undefined : plan.seats[seat]?.label;
    if (seat !== undefined) {
      trial.expectedAnchor = tableAnchor(plan, seat, volume.id);
    }
    process.stderr.write(
      `[death-loop] ${volume.id}: respawned at ` +
        `${trial.respawnPos ? `[${trial.respawnPos.map((n) => n.toFixed(2)).join(", ")}]` : "?"} ` +
        `(${trial.respawnSeat ?? "NO declared seat"})\n`,
    );

    if (stake === undefined) return;
    const objective = stake.currency.objective;

    // --- the forfeit -------------------------------------------------------
    if (trial.balanceBefore !== undefined) {
      trial.balanceAfterDeath = await this.settledScore(
        objective,
        trial.balanceBefore - (trial.expectedForfeit ?? 0),
      );
    }

    // --- the walk back, and the stake at the end of it ----------------------
    const anchor = trial.expectedAnchor;
    if (anchor === undefined) return;
    try {
      await this.walkTo(anchor, 1, `death-loop walk back to the ${stake.id} at the near lip`);
      trial.walkedBack = true;
    } catch (err) {
      process.stderr.write(
        `[death-loop] ${volume.id}: the walk back to [${anchor.join(", ")}] failed: ` +
          `${err instanceof Error ? err.message : String(err)}\n`,
      );
      return;
    }
    await this.awaitEntitySettle();
    const marker = this.stakeHardwareAt(anchor);
    trial.markerPos = marker ? [marker.position.x, marker.position.y, marker.position.z] : undefined;
    if (!marker) return;

    // --- the collection, twice in one tick ---------------------------------
    // AC6 is *idempotent under a double right-click in one tick*, so the two
    // clicks are issued back to back in one event-loop turn — the client's own way
    // of putting two `use_entity` packets on the wire inside one server tick — and
    // the assertion is on the OUTCOME: the purse must end at exactly what it held
    // before the death, never twice the stake.
    //
    // **Stated as the limitation it is** (playtest-methodology rule 1: a gate must
    // not claim more than it bound). Whether the server ADJUDICATED both clicks in
    // one tick is not observable from a client, and measurement says it usually
    // does not: the collect rides an interaction advancement, and vanilla grants an
    // advancement at most once per tick, so the second packet is normally absorbed.
    // `collect_clicks` therefore counts packets SENT, not collections adjudicated,
    // and this exercise is best-effort. What is NOT best-effort is the outcome it
    // is checked against — a purse that grew twice, or a marker still standing
    // afterwards, is caught either way, and the second is what actually caught a
    // deliberately non-idempotent `stk_take` in the red demonstration.
    const target = bot.entities[marker.id];
    if (target) {
      const eye = bot.entity.position;
      try {
        await bot.lookAt(
          eye.offset(
            marker.position.x - eye.x,
            marker.position.y - eye.y,
            marker.position.z - eye.z,
          ),
          true,
        );
      } catch {
        // a look failure must not be reported as a collection failure
      }
      const clicks = [bot.activateEntity(target), bot.activateEntity(target)];
      trial.collectClicks = clicks.length;
      await Promise.allSettled(clicks);
    }
    trial.balanceAfterCollect =
      trial.balanceBefore === undefined
        ? this.myScore(objective)
        : await this.settledScore(objective, trial.balanceBefore);
    trial.markerRetired = await this.waitFor(
      () => this.stakeHardwareAt(anchor) === undefined,
      MARKER_RETIRE_TIMEOUT_MS,
      LEDGER_POLL_MS,
    );
    process.stderr.write(
      `[death-loop] ${volume.id}: collected — ledger ${trial.balanceAfterDeath ?? "?"} → ` +
        `${trial.balanceAfterCollect ?? "?"}; marker ` +
        `${trial.markerRetired ? "retired" : "STILL STANDING"}\n`,
    );
  }

  /**
   * Put the bot's feet inside `box`, having already walked to its lip.
   *
   * The pathfinder cannot be asked for this: its nearest goal is a range of one
   * block, so it parks the bot beside a one-cell hazard and calls it arrived. Raw
   * forward drive is the mechanism this file already trusts against exact cells
   * (the timed-gate dash, the unstick burst) and it is what a player pressing W
   * does. Throws whatever the walk threw — including the {@link BotDeathError}
   * that is the whole point.
   *
   * It also RECORDS what it saw. The drive can run its deadline out with the body
   * still outside the box — a wall in the way, a cell no body fits in — and it
   * returns normally when it does, so the only thing separating "the volume did
   * not kill what was in it" from "nothing ever got in" is this flag.
   */
  private async stepInto(box: Box, cell: Vec3Tuple, trial: LethalTrial): Promise<void> {
    const bot = this.requireBot();
    const inside = (): boolean => {
      if (!this.bodyInside(box)) return false;
      trial.enteredVolume = true;
      return true;
    };
    if (inside()) return;
    const deadline = Date.now() + LETHAL_DEATH_TIMEOUT_MS;
    try {
      while (Date.now() < deadline && !inside()) {
        if (this.death) throw this.death;
        const p = bot.entity.position;
        try {
          await bot.lookAt(p.offset(cell[0] + 0.5 - p.x, 0, cell[2] + 0.5 - p.z), true);
        } catch {
          // best effort — a look failure must not abort the step
        }
        bot.setControlState("forward", true);
        await delay(GATE_DASH_TICK_MS);
      }
    } finally {
      bot.clearControlStates();
    }
  }

  /**
   * Whether the volume `box` would match this body right now — the SERVER's rule
   * ({@link bodyInVolume}), asked of the bot's exact position.
   *
   * It asked whether the floored feet CELL was in the box, which is a different
   * question and a narrower one: the emitted selector intersects a 0.6-wide
   * hitbox against the region `[lo, hi + 1]`, so a body 0.2 blocks outside the
   * face is one the volume kills and one this used to call outside. Saying
   * "entered" of exactly the bodies the volume can act on is what makes
   * `enteredVolume` mean anything, and it also ends {@link stepInto}'s drive at
   * the moment the hazard can reach the bot rather than a third of a block late.
   */
  private bodyInside(box: Box): boolean {
    const p = this.bot?.entity?.position;
    return p !== undefined && bodyInVolume([p.x, p.y, p.z], box);
  }

  /**
   * Whether a body could BE in `cell` — its own cell and the one above it clear.
   *
   * Deliberately weaker than {@link stanceStandable}: a lethal volume is often a
   * hole, and falling into one is exactly how a player meets it, so demanding
   * solid support underfoot would rule out the cells the volume is made of. What
   * it does rule out is a cell filled by a block, which no walk can ever reach.
   *
   * Block-shape based (`boundingBox`), like {@link gateOpen}, so it stays right
   * for whatever the campaign built with. A cell whose chunk is not loaded reads
   * as occupiable: the conservative direction here is to keep a candidate the
   * approach can then be measured against, never to silently narrow the box.
   */
  private bodyCanOccupy(cell: Vec3Tuple): boolean {
    const bot = this.bot;
    if (!bot?.entity) return true;
    const p = bot.entity.position;
    const at = (dy: number) => bot.blockAt(p.offset(cell[0] - p.x, cell[1] + dy - p.y, cell[2] - p.z));
    const feet = at(0);
    const head = at(1);
    if (!feet || !head) return true;
    return feet.boundingBox === "empty" && head.boundingBox === "empty";
  }

  /**
   * The recovery stake's own hardware standing at `anchor`: the `interaction` box
   * a player right-clicks, provided the glowing `item_display` that says there is
   * something here is standing with it.
   *
   * Both halves are required, and that is the assertion rather than a convenience:
   * spec-0032 declares the stake to be an interaction for the hitbox AND a glowing
   * item display for the rendering, so an invisible hitbox is a stake no player
   * would ever find, not a stake that happens to render oddly.
   *
   * WHICH of them is the stake is {@link markerAt}'s rule, not this method's — the
   * executor supplies the observation and the pure module decides, so the reading
   * can be put to a test without a server in front of it.
   */
  private stakeHardwareAt(anchor: Vec3Tuple): Hitbox | undefined {
    const bot = this.bot;
    if (!bot) return undefined;
    const displays = Object.values(bot.entities).flatMap((e) =>
      e?.position && e.name === "item_display"
        ? [{ name: "item_display", position: { x: e.position.x, y: e.position.y, z: e.position.z } }]
        : [],
    );
    return markerAt(
      this.hitboxesNear(anchor, MARKER_SEARCH_RADIUS),
      displays,
      anchor,
      MARKER_SEARCH_RADIUS,
      MARKER_PAIR_RADIUS,
    );
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
    // Deliberately NOT remembered for replay. `class_apply_<class>` ends in
    // `teleport @s <campaign entry point>`, so re-running this after a death moves
    // the bot off the very respawn point the die-retry stage exists to measure.
    // The post-death re-arm is `rearmAfterRespawn`, which only puts
    // the kept kit back on.
    // The class-selection dialog button runs `step.command` (a `/trigger`); the
    // bot fires the same command directly. The per-tick handler then applies the
    // kit and teleports the player to the campaign spawn.
    bot.chat(step.command);
    // Give the datapack a tick to reset the trigger, give the kit and teleport.
    await delay(CLASS_SETTLE_MS);
    // Equip the kit (sword + armor) so the bot can fight v0.3 combat waves. A
    // no-op for kits without those items.
    await this.equipLoadout();
    // The baseline every later `keep_inventory` judgement is made against: what a
    // living, kitted bot carries.
    this.itemsBeforeDeath = bot.inventory.items().length;
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
    // Walk to the NPC first (realism; some dialog effects are reach-gated), then
    // chat the dialog-option `/trigger` command the button would have run. The
    // trigger is sent HOWEVER the walk ended — arriving is the means, the trigger is
    // the step — and `chatTrigger` records the server's answer so a step that then
    // times out can name which side swallowed it.
    await this.walkTo(step.pos, TALK_RANGE, `npc ${step.npc}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
    // The trigger stands in for a dialog button this bot cannot click — so before
    // it is sent, prove the button was REACHABLE: cast the crosshair ray a player
    // casts and require this NPC's own hitbox to be what it meets first. Without
    // this, a second body on the NPC's cell is invisible to the machine and fatal
    // to the player (owner island QA, terminal finding).
    this.requireCrosshair(step.pos, `talk-to ${step.npc}`, TALK_RANGE);
    this.chatTrigger(step.command);
    // A dialogue that OPENED proves nothing: the option must actually complete the
    // objective this step stands for. Wait for that objective's own marker.
    await this.requireObjective(step.objective, `talk-to ${step.npc}`);
    await delay(EFFECT_SETTLE_MS);
  }

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
  async reach(step: ReachStep): Promise<void> {
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
  }

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
  private async walkTo(
    pos: readonly [number, number, number],
    range: number,
    label: string,
    sneak = false,
    completion?: StepCompletion,
  ): Promise<void> {
    const bot = this.requireBot();
    const r = Math.max(1, Math.floor(range));
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
      // spec-0016 §4: the timed gates this leg's proven route walks
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
  }

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
  private stepSettled(completion: StepCompletion): string | undefined {
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
  }

  /**
   * Whether the bot currently stands at/near a compiler-exported transport
   * destination — the same arrival predicate {@link awaitTransport} uses, so the
   * mid-walk check and the post-step settle can never disagree about "arrived".
   */
  private atTransportDest(dest: readonly [number, number, number]): boolean {
    const bot = this.bot;
    if (!bot?.entity) return false;
    const p = bot.entity.position;
    return (
      Math.abs(p.x - (dest[0] + 0.5)) < TRANSPORT_NEAR &&
      Math.abs(p.z - (dest[2] + 0.5)) < TRANSPORT_NEAR &&
      Math.abs(p.y - dest[1]) < 4
    );
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
   * the window rather than its tail (spec-0016 §4).
   *
   * Two bounded phases: first watch until the gates read CLOSED (so an already-open
   * window whose remaining ticks are unknown is not mistaken for a fresh one), then
   * watch until they read OPEN. Each phase is capped at {@link gateWindowWaitMs} —
   * one full cycle plus margin, within which the clock is guaranteed to produce the
   * edge — so an unreadable region (chunk not loaded) can never hang the run; the
   * wait simply gives up and the caller tries the hop anyway.
   *
   * Returns `true` iff the fresh closed→open edge was actually OBSERVED (both
   * phases succeeded). `false` means the caller knows nothing about the clock —
   * safe to try on a gate that merely blocks, forbidden on one that crushes.
   *
   * `hold`: a feet cell to actively KEEP while watching. The tide-mill
   * gate corridor is flowing water; a bot that merely idles is carried off the
   * staging mouth by the current (observed: 8 blocks back to the pool during one
   * 4-second wait), and every drifted block must be re-walked inside the open
   * window. A dynamic pathfinder goal pinned to the cell is the bot's equivalent
   * of a player holding a movement key against the tide. Cleared before returning,
   * so the crossing `goto` starts from clean pathfinder state.
   *
   * This is navigation, not game logic: the harness reads the world only to TIME a
   * movement the compiler already proved possible. It asserts nothing about the gate.
   */
  private async waitForGateWindow(
    gates: readonly TimedGate[],
    hold?: Vec3Tuple,
    press?: Vec3Tuple,
  ): Promise<boolean> {
    const bot = this.requireBot();
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
    // Station-keeping with RAW controls, once per poll. A dynamic pathfinder goal
    // was tried first and lost ~4 blocks per wait: the pathfinder "arrives", clears
    // its controls, the current takes the idle bot, and the re-solve lags the
    // drift. Raw look-and-walk is the mechanism this file already trusts against
    // clocked geometry (the unstick path, the-drowned-bell's crossing burst):
    // face the station, hold forward while off it, release on it. Drift downstream
    // is corrected against the current the same way the proven approach hops walk
    // it; an overshoot past the mouth is carried back by the very current that
    // caused it (and the shut gate is a solid wall — the region cannot be entered
    // while it matters).
    //
    // With `press` (crush staging), a gate that provably reads CLOSED upgrades the
    // stance: lean INTO the shut plane toward the crossing target, so the open
    // edge releases a bot that is already in contact and already moving. Solid
    // blocks make the lean safe; the moment the state is open (or unknown), the
    // stance falls back to holding the mouth.
    const keepStation = async (): Promise<void> => {
      if (press && allOpen() === false) {
        const p = bot.entity.position;
        try {
          await bot.lookAt(p.offset(press[0]! + 0.5 - p.x, 0, press[2]! + 0.5 - p.z), true);
        } catch {
          // best effort — a look failure must not abort the watch
        }
        bot.setControlState("sprint", true);
        bot.setControlState("forward", true);
        return;
      }
      if (!hold) return;
      const p = bot.entity.position;
      const dx = hold[0]! + 0.5 - p.x;
      const dz = hold[2]! + 0.5 - p.z;
      if (dx * dx + dz * dz > GATE_HOLD_SLACK_SQ) {
        try {
          await bot.lookAt(p.offset(dx, 0, dz), true);
        } catch {
          // best effort — a look failure must not abort the watch
        }
        bot.setControlState("forward", true);
      } else {
        bot.setControlState("forward", false);
        bot.setControlState("sprint", false);
      }
    };
    const watch = async (want: boolean, phase: string): Promise<boolean> => {
      const deadline = Date.now() + cap;
      while (Date.now() < deadline) {
        if (allOpen() === want) return true;
        await keepStation();
        await delay(GATE_POLL_MS);
      }
      process.stderr.write(
        `[timed-gate] gave up waiting for the gate to read ${phase} after ` +
          `${(cap / 1_000).toFixed(1)}s — crossing on the next attempt regardless\n`,
      );
      return false;
    };
    try {
      if (!(await watch(false, "closed"))) return false;
      process.stderr.write(`[timed-gate] gate is shut; waiting for it to open\n`);
      if (await watch(true, "open")) {
        process.stderr.write(`[timed-gate] window open — crossing now\n`);
        return true;
      }
      return false;
    } finally {
      if (hold || press) {
        bot.clearControlStates();
      }
    }
  }

  /**
   * Raw-control crossing dash for a `crush: true` gate entry (see
   * {@link GateAssist.dash}). Face the far mouth and drive forward (sprinting)
   * from the near mouth until the far mouth is reached, re-aiming every
   * {@link GATE_DASH_TICK_MS}. No pathfinder anywhere: the tide-mill corridor
   * floods through the freshly opened plane, and a pathfinder start (plus its
   * mid-water replans) measured slower than the 1.8 s window — while raw forward
   * drive is exactly how the proven approach hops already beat the same current.
   *
   * Bounded by `budgetMs`: if it expires with the bot still inside a gate region,
   * the dash REVERSES raw toward `from` (the current helps — it points out the
   * near side) so the closing edge finds the bot outside the fill, and reports
   * failure for the caller to take the next window. Never a check weakened: a
   * dash that cannot clear still fails its attempt loudly.
   */
  private async dashThroughGate(
    through: readonly TimedGate[],
    from: Vec3Tuple,
    to: Vec3Tuple,
    budgetMs: number,
  ): Promise<boolean> {
    const bot = this.requireBot();
    const inside = (): boolean => {
      const feet = this.feetCell();
      return feet !== undefined && through.some((g) => insideGate(feet, g));
    };
    const driveToward = async (cell: Vec3Tuple): Promise<void> => {
      const p = bot.entity.position;
      try {
        await bot.lookAt(p.offset(cell[0]! + 0.5 - p.x, 0, cell[2]! + 0.5 - p.z), true);
      } catch {
        // best effort — a look failure must not abort the dash
      }
      bot.setControlState("sprint", true);
      bot.setControlState("forward", true);
      // Deliberately NO jump: measured live in the tide-mill race (1–2 deep
      // flowing water), holding jump turns the drive into an upward swim that
      // lifted the bot into the fill plane at head height — slower AND lethal.
      // The press stance (see waitForGateWindow) is what buys the crossing time.
    };
    try {
      bot.clearControlStates();
      const deadline = Date.now() + Math.max(GATE_DASH_TICK_MS * 4, budgetMs);
      while (Date.now() < deadline) {
        const p = bot.entity.position;
        const dx = to[0]! + 0.5 - p.x;
        const dz = to[2]! + 0.5 - p.z;
        // Arrived on the far mouth, out of the fill: crossed.
        if (dx * dx + dz * dz <= GATE_DASH_ARRIVE_SQ && !inside()) {
          return true;
        }
        await driveToward(to);
        await delay(GATE_DASH_TICK_MS);
      }
      if (!inside()) {
        // Out of budget but already clear of every region — let the caller's goto
        // finish (or fail) the hop; nothing here is in the fill's path.
        process.stderr.write(
          `[timed-gate] dash out of window at ${fmt(bot.entity.position)} (clear of the ` +
            `fill; aiming for [${to.join(", ")}])\n`,
        );
        return false;
      }
      // Emergency: still inside the fill with the window spent. Reverse OUT the
      // near side — with the corridor current, retreat is downhill.
      process.stderr.write(
        `[timed-gate] dash out of window while inside the gate — retreating raw to ` +
          `[${from.join(", ")}]\n`,
      );
      const retreatDeadline = Date.now() + GATE_DASH_RETREAT_MS;
      while (Date.now() < retreatDeadline && inside()) {
        await driveToward(from);
        await delay(GATE_DASH_TICK_MS);
      }
      return false;
    } finally {
      bot.clearControlStates();
    }
  }

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
    // stopped before it could be completed" (after a physics-unstick lands
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
        // Through the navigation owner, never `bot.pathfinder.goto` directly: this
        // wait is abandoned on a death and on the timeout, and an abandoned trip's
        // later rejection has to land on a handler that already exists.
        await this.raceDeath(() =>
          withTimeout(
            this.nav.goto(new goals.GoalNear(x, y, z, range)),
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
   * entities: NPC puppets and staged story actors are summoned as ordinary living
   * mobs and can sit right where combat happens. `nearestEntity` cannot tell them
   * from a wave mob by shape, and mineflayer on 1.21.11 cannot read the entity
   * `Tags`/scoreboard that would (the `KillStep.tag` is informational only). So the
   * bot proves the wave down without ever attacking — or walking to — a body the
   * delve says is not a fight:
   *   * the kinds this delve stages as NPCs are excluded from targeting outright,
   *     off `critical-path.json`'s `non_combatants` (see {@link isWaveMob});
   *   * a confirmed KILL is a targeted mob that winks out in melee near the anchor.
   *     That tally is a DIAGNOSTIC and no longer a terminal condition: what ends the
   *     step is the wave census, the server's own answer over the wave's tag (see
   *     `wave.ts`). A tally reaching `step.count` still asks the census at once, so
   *     the bot leaves the moment the fight is provably over and never treks off to
   *     a distant staged actor (which would trip later-area triggers);
   *   * a candidate that outlives the melee budget the ENCOUNTER's own arithmetic
   *     gives its kind (`bodies[].give_up_swings`), or that cannot be pathed to, is
   *     dropped so the loop hunts the next real wave mob instead of fixating —
   *     and a body dropped for outliving its budget is a FINDING the run report
   *     names, never a blacklist this file invented. When no eligible wave mob is
   *     left, the wave is likewise cleared.
   * Navigation + assertion only: the datapack's kill advancement + countdown are what
   * actually complete the objective when the last tagged mob dies.
   */
  private async fightWave(step: KillStep): Promise<void> {
    const bot = this.requireBot();
    // Confirmed kills: a mob the bot has attacked that then vanishes near the wave
    // anchor (see wave.ts). Counting these (rather than "no mob-shaped entity remains")
    // is what lets the step end at `step.count` without walking to a far Invulnerable
    // actor. Armed BEFORE the approach walk: self-defense can kill a wave mob
    // that ambushes the bot on the way in, and that kill is wave progress like any other
    // — crediting it only from the kill loop deadlocked the objective (ladder run 13).
    const engagement = beginWave(step.wave, step.pos);
    // What the SERVER has said about this wave during this step. The terminal
    // condition, and the source of the fight's attribution. See `wave.ts`: a body
    // that dies in a way the proximity rule cannot attribute, or a scripted death
    // that re-seats the wave under a private counter, both leave `killed` unable
    // ever to reach `step.count` — measured on the gallery, where two of three
    // bodies withered and fell and `1/3` was as far as the tally could get.
    const watch = beginCensusWatch();
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
    // Kinds this encounter states no budget for — named in the timeout message,
    // which is thrown outside the loop, because "the bot gave up on nothing" and
    // "there was nothing to give up on" are different facts.
    const unbounded = new Set<string>();
    try {
      await this.equipLoadout();
      await this.walkTo(step.pos, 3, `wave ${step.wave}`, step.sneak, {
        objective: step.objective,
        transport: step.transport,
      });
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
      // Swings landed on each body this step, so a body can be held to the budget
      // its own kind was given. Counting SWINGS rather than seconds is the
      // compiler's own choice of unit for this arithmetic: swing damage is
      // Mojang's item data, while timing depends on charge discipline nothing
      // here models.
      const swings = new Map<number, number>();
      // The wave's own census: every "the fight is over" test below is
      // a guess made from SHAPES, and the server can simply be asked. See
      // {@link pollWaveCensus}.
      const enc = this.encounterFor(step.wave);
      let lastCensusAt = 0;
      const askCensus = async (): Promise<number | undefined> => {
        lastCensusAt = Date.now();
        return this.pollWaveCensus(step, enc, watch);
      };
      while (Date.now() < deadline) {
        // Fail fast if a mob killed the bot mid-fight (gap 7) rather than looping.
        if (this.death) throw this.death;
        // THE terminal condition: the server says nothing of this wave stands.
        // Asked on its own cadence, and at once when the bot's own tally has
        // reached the wave's declared size — which is the only thing that tally
        // decides now.
        const floorMs = engagement.killed >= step.count ? 0 : WAVE_CENSUS_POLL_MS;
        if (Date.now() - lastCensusAt >= floorMs) {
          await askCensus();
          if (censusCleared(watch)) {
            process.stderr.write(
              `[kill ${step.wave}] the wave census reports nothing of ${step.wave} standing, ` +
                `on ${watch.clearStreak} consecutive answers (${watch.peakStanding} standing ` +
                `at its fullest this step; the bot confirmed ${engagement.killed} of ` +
                `${step.count} itself) — wave cleared\n`,
            );
            return;
          }
        }
        // Eat between exchanges when hurt and nothing is in reach (no-op otherwise).
        await this.maybeEat(`wave ${step.wave}`);
        const cast = this.requireNonCombatants();
        const wave = bot.nearestEntity(
          (e) => isWaveMob(e, bot.entity, cast) && !blacklist.has(e.id),
        );
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
            if (!((await askCensus()) ?? 0)) {
              process.stderr.write(
                `[kill ${step.wave}] every mob this fight engaged is down ` +
                  `(${engagement.killed} confirmed near the anchor) and no hostile is within ` +
                  `${WAVE_ENGAGE_NEAR} blocks — wave cleared\n`,
              );
              return;
            }
            // The census overruled the guess. Start the streak over rather than
            // asking again on the next poll: a census is a server round-trip, and
            // a wave that is standing somewhere unreachable would otherwise be
            // interrogated several times a second until the budget ran out.
            clearedStreak = 0;
          }
          await delay(REACH_POLL_MS);
          continue;
        }
        clearedStreak = 0;
        if (!mob) {
          // No eligible wave mob remains (every real mob dead; any unkillable actor
          // blacklisted) → wave cleared, unless the census can still see it. When it
          // can, there is nothing this loop can do about it — the survivor is out of
          // reach or unkillable — so the step still ends, but it ends having SAID so,
          // instead of reporting a clearance the objective will contradict.
          if (++emptyStreak >= WAVE_CLEAR_STREAK) {
            if (((await askCensus()) ?? 0) > 0) {
              process.stderr.write(
                `[kill ${step.wave}] nothing eligible is left to attack, but the wave census ` +
                  `still counts mobs alive — leaving the fight unfinished rather than claiming ` +
                  `it won\n`,
              );
            }
            return;
          }
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
          }
          // Every mob the bot melees during this step is recorded, retaliation target or
          // not. Excluding retaliation targets would keep a non-wave stalker from
          // inflating the count, but it is over-broad — a WAVE mob that attacks the bot
          // is picked by the retaliation rule too, and refusing to credit it makes the
          // objective impossible to finish (ladder run 13). The proximity rule in
          // {@link creditsWaveKill} is the arbiter, for a self-defense kill exactly as
          // for one the kill loop targeted.
          engagement.engaged.add(mob.id);
          await bot.lookAt(mob.position.offset(0, (mob.height ?? 1) * 0.5, 0), true);
          bot.attack(mob);
          const landed = (swings.get(mob.id) ?? 0) + 1;
          swings.set(mob.id, landed);
          await delay(ATTACK_INTERVAL_MS);
          const budget = giveUpBudgetFor(enc, mob.name);
          if (budget === undefined) {
            // The encounter states no budget for this kind, so there is nothing
            // to hold the body to. The bot keeps swinging until the step's own
            // budget expires, and the timeout says which kinds it could not
            // judge — never a fallback to a number nobody derived.
            unbounded.add(mob.name ?? "?");
          } else if (landed >= budget) {
            // The body outlived the arithmetic that says how long it should
            // take. That is a content defect, and the report NAMES it — the bot
            // stops swinging so the run can go on, which is a different act from
            // deciding the body is scenery.
            blacklist.add(mob.id);
            engagedId = undefined;
            this.recordUnkillable(step.wave, mob.name ?? "?", landed, budget);
          }
        }
      }
    } finally {
      bot.removeListener("entityGone", onGone);
      this.activeWave = undefined;
    }
    throw new Error(
      `kill timed out after ${KILL_TIMEOUT_MS}ms: wave ${step.wave} not cleared — the census ` +
        `last reported ${watch.standing ?? "no"} of the wave standing over ${watch.answers} ` +
        `answer(s)${watch.seen ? "" : ", and never once saw the wave exist"} ` +
        `(the bot confirmed ${engagement.killed}/${step.count} itself; ` +
        `${engagement.engaged.size} mob(s) engaged)` +
        unboundedEncounterNote(unbounded),
    );
  }


  /**
   * Adopt the compiler's combat plan (spec-0023). With it, a `kill` step becomes a
   * verified ENCOUNTER rather than a fight to be won: the die-retry stage proves
   * dying is safe, the assist windows keep bot fencing skill from capping how hard
   * a delve may be, and a billed `elite`/`boss` gets one honest unassisted attempt
   * so the inverted floor gate has something to measure.
   */
  /**
   * Adopt the delve's cast statement — the kinds nothing may swing at.
   *
   * Called by the entrypoint before the run starts, from the parsed critical
   * path. An empty set is a legitimate value (a campaign with no NPCs); NOT
   * calling this at all is a harness wiring bug, and {@link requireNonCombatants}
   * refuses rather than guessing.
   */
  useNonCombatants(kinds: ReadonlySet<string>): void {
    this.nonCombatants = kinds;
  }

  /** The cast statement, or a refusal naming what is missing. */
  private requireNonCombatants(): ReadonlySet<string> {
    if (!this.nonCombatants) {
      throw new Error(
        "the executor was never told which entity kinds are never a combat target. That " +
          "statement is `non_combatants` in `critical-path.json` (contract format 4) and the " +
          "entrypoint must hand it over before the run starts — there is no default, because " +
          "the only available default is a list of entity names written in the harness, which " +
          "is right only for the campaigns whose author happened to pick those bodies",
      );
    }
    return this.nonCombatants;
  }

  useCombatPlan(plan: CombatPlan, dieRetry: boolean, actorFloorGate = true): void {
    this.combatPlan = plan;
    this.dieRetry = dieRetry;
    this.actorFloorGate = actorFloorGate;
  }

  /** The objectives the compiled path proves — what decides which actor fights
   * this run can reach at all. Set by the entrypoint before the run starts. */
  usePathObjectives(objectives: Iterable<string>): void {
    this.pathObjectives = new Set(objectives);
  }

  /** Every actor fight this run attempted, for the run report. */
  actorFightTrials(): readonly ActorTrial[] {
    return this.actorTrials;
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

  /**
   * Who felled `wave`'s bodies, as its last census answered.
   *
   * `unattributed` when no census answered during the fight — which is a fact
   * about the probe, and must never be readable as a clean win.
   */
  waveAttribution(wave: string): FightAttribution {
    return (
      this.waveAttributions.get(wave) ?? {
        kind: "unattributed",
        reason: "no wave census answered during this run's fight at this encounter",
      }
    );
  }

  /** Bodies that did not fall inside their encounter's own melee budget. */
  unkillableFindings(): readonly string[] {
    return this.unkillableBodies.map(unkillableFinding);
  }

  /** Note a body the encounter's arithmetic says should have fallen. */
  private recordUnkillable(
    wave: string,
    kind: string,
    swings: number,
    budget: number,
  ): void {
    // One line per KIND per wave: a wave of eight of them is one defect,
    // and eight identical findings would bury the seven other things the
    // report has to say.
    if (this.unkillableBodies.some((u) => u.wave === wave && u.kind === kind)) return;
    const finding = { wave, kind, swings, budget };
    this.unkillableBodies.push(finding);
    process.stderr.write(`[kill ${wave}] ${unkillableFinding(finding)}\n`);
  }

  /** Every named-entity death this run observed, raw and unclassified — the
   * entrypoint classifies each scripted-teardown-vs-combat for the run report. */
  namedEntityDeaths(): readonly NamedEntityDeath[] {
    return this.namedEntityDeathLog;
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
      this.stageNow = "die-retry";
      try {
        await this.dieRetryAt(step, enc);
      } finally {
        this.stageNow = "critical-path";
      }
    }
    if (assistPolicy(enc) === "unassisted-first") {
      this.encounterPhases.set(enc.wave, "unassisted");
      const won = await this.attemptUnassisted(step, enc);
      // The attribution the attempt's own last census gave. `unattributed` only
      // when no census answered during it, which is a fact about the probe rather
      // than about the fight, and says so.
      const finding = floorFinding(
        enc,
        { attempted: true, won },
        this.waveAttribution(enc.wave),
      );
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
   * The actor floor gate, fired once per actor after the objective that
   * unleashes it completes.
   *
   * Telemetry, never a gate: nothing on the critical path depends on the outcome,
   * so every failure mode here — the body never appearing, a lost fight, a
   * timeout, even the bot dying — is RECORDED and the run continues. An actor
   * fight blocks no objective, which is also why it takes **no assist**: the
   * assist exists to stop bot fencing skill capping how hard a delve may be on a
   * fight the run must finish, and there is no such obligation here. Losing is a
   * perfectly good souls answer.
   */
  private async actorFloorGateAfter(objectiveId: string): Promise<void> {
    const actors = this.combatPlan?.actors ?? [];
    if (actors.length === 0) return;
    for (const a of actors) {
      if (this.actorsEngaged.has(a.actor)) continue;
      const decision = actorExercise(a, this.pathObjectives);
      if (decision.kind !== "exercise" || decision.afterObjective !== objectiveId) continue;
      this.actorsEngaged.add(a.actor);
      if (!this.actorFloorGate) {
        process.stderr.write(
          `[actor] ${a.actor}: skipped via DELVEWRIGHT_ACTOR_FLOOR=0 — the report records it ` +
            `as skipped, never as measured\n`,
        );
        continue;
      }
      const trial = await this.fightActor(a, objectiveId);
      this.actorTrials.push(trial);
      const finding = actorFloorFinding(trial, actorAttribution());
      if (finding) {
        this.floorFindings.push(finding);
        process.stderr.write(`[floor] ${finding}\n`);
      }
    }
  }

  /**
   * One honest, unassisted attempt at a tiered actor's unleashed body.
   *
   * The body is identified the way the plan describes it — the actor's own entity
   * type, near the anchor cell the compiler resolved, preferring the one wearing
   * its custom name. Entity TAGS are the compiler's real identity for it, but a
   * client cannot read tags, so this is the closest a bot can honestly get; a
   * body it cannot find is reported as `body-not-found` rather than counted as a
   * win, because "nothing was there" and "I beat it" must never share a row.
   */
  private async fightActor(a: ActorEncounter, afterObjective: string): Promise<ActorTrial> {
    const started = Date.now();
    let swings = 0;
    const record = (outcome: ActorOutcome, detail?: string): ActorTrial => ({
      actor: a.actor,
      tier: a.tier,
      afterObjective,
      outcome,
      swings,
      elapsedMs: Date.now() - started,
      detail,
    });
    const pos = a.pos!;
    process.stderr.write(
      `[actor] ${a.actor} is billed \`${a.tier}\` (${a.entity}` +
        `${a.maxHealth !== undefined ? `, ${a.maxHealth} hp` : ""}) and \`${afterObjective}\` ` +
        `unleashed it — one unassisted attempt at ${pos.join(",")}\n`,
    );
    try {
      await this.walkTo(pos, 3, `actor ${a.actor}`);
      const bot = this.requireBot();
      const body = await this.findActorBody(a);
      if (body === undefined) {
        const why =
          `no live \`${a.entity}\` within ${ACTOR_MATCH_RADIUS} blocks of ${pos.join(",")} ` +
          `after ${ACTOR_SETTLE_MS}ms — the unleash beat may not have fired, or the twin was ` +
          `summoned elsewhere`;
        process.stderr.write(`[actor] ${a.actor}: ${why}\n`);
        return record("body-not-found", why);
      }
      const id = body.id;
      const deadline = Date.now() + ACTOR_FIGHT_TIMEOUT_MS;
      while (Date.now() < deadline) {
        if (this.death) throw this.death;
        const live = bot.entities[id];
        if (!live?.position) {
          process.stderr.write(
            `[actor] ${a.actor}: DOWN after ${swings} swing(s) — the unassisted bot won cold\n`,
          );
          return record("won-first-try");
        }
        await this.maybeEat(`actor ${a.actor}`);
        const dist = bot.entity.position.distanceTo(live.position);
        if (dist > 3) {
          await this.walkTo(
            [Math.floor(live.position.x), Math.floor(live.position.y), Math.floor(live.position.z)],
            2,
            `actor ${a.actor}`,
          );
          continue;
        }
        await bot.lookAt(live.position.offset(0, (live.height ?? 1) * 0.5, 0), true);
        bot.attack(live);
        swings += 1;
        await delay(ATTACK_INTERVAL_MS);
      }
      const why = `still standing after ${ACTOR_FIGHT_TIMEOUT_MS}ms and ${swings} swing(s)`;
      process.stderr.write(`[actor] ${a.actor}: ${why}\n`);
      return record("timed-out", why);
    } catch (err) {
      const detail = err instanceof Error ? err.message : String(err);
      process.stderr.write(`[actor] ${a.actor}: the unassisted attempt ended — ${detail}\n`);
      // A death here is the encounter doing its job, not a failed run: recover the
      // bot the same way a lost wave attempt does and let the path carry on.
      if (this.death) await this.respawnAndRearm();
      return record("lost", detail);
    }
  }

  /**
   * The actor's live body: the nearest entity of its declared type within
   * {@link ACTOR_MATCH_RADIUS} of the anchor cell, waiting up to
   * {@link ACTOR_SETTLE_MS} for the summon to land and entity tracking to catch up.
   * A custom-named actor prefers the body wearing that name.
   */
  private async findActorBody(a: ActorEncounter): Promise<{ id: number } | undefined> {
    const bot = this.requireBot();
    const want = a.entity.replace(/^minecraft:/, "");
    const pos = a.pos!;
    const deadline = Date.now() + ACTOR_SETTLE_MS;
    for (;;) {
      const near = Object.values(bot.entities)
        .filter((e) => e?.position && e.name === want)
        .map((e) => {
          const label = displayNameOf(e);
          return {
            id: e.id,
            label,
            named: label === a.name,
            d: Math.hypot(e.position.x - pos[0], e.position.y - pos[1], e.position.z - pos[2]),
          };
        })
        .filter((e) => e.d <= ACTOR_MATCH_RADIUS)
        .sort((x, y) => Number(y.named) - Number(x.named) || x.d - y.d);
      const best = near[0];
      if (best) {
        // spec-0029: the preference is measured every time it is exercised —
        // how many bodies it chose between and how many of them had a name the
        // heuristic could read. Recorded on the deciding pass only, so the
        // settle-loop's earlier empty polls do not inflate the count.
        this.namePreferenceDecisions += 1;
        this.namePreferenceCandidates += near.length;
        const named = near.filter((e) => e.label !== undefined && e.label !== "").length;
        this.namePreferenceNamedCandidates += named;
        if (named > 0) this.namePreferenceWithName += 1;
        return { id: best.id };
      }
      if (Date.now() >= deadline) return undefined;
      await delay(REACH_POLL_MS);
    }
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
    // never report a passed die-retry.
    // PRECONDITION: the loop this stage proves is
    // "death → respawn at the governing checkpoint → walk back". Two ways that
    // premise can be false, and they are different kinds of fact:
    //   * the checkpoint exists but was never ARMED (a bonfire the route walked
    //     past) — the harness's own gap, and every measurement below would
    //     describe it rather than the delve. Red;
    //   * the campaign fires NO checkpoint before this fight at all — a content
    //     fact: every death here is a full restart. Advisory, because the
    //     compiler's retry-cost and checkpoint rules are what judge that.
    // Either way: take NO death. A scripted death would blame the campaign for a
    // proof that was never in a position to be made.
    const precondition = checkpointPrecondition(
      enc,
      this.restSteps,
      this.restedBonfires,
      this.currentStep,
    );
    if (precondition !== undefined) {
      (precondition.reds ? this.preconditionFindings : this.preconditionAdvisories).push(
        precondition.finding,
      );
      this.preconditionWaves.add(enc.wave);
      process.stderr.write(`[die-retry] ${precondition.finding}\n`);
      return;
    }
    this.dieRetryEngaged.add(enc.wave);
    // ASSISTED. The approach walks the bot to within 3 blocks of a
    // LIVE encounter — melee range — and until now it did so with nothing on. That
    // made bot fencing skill the gate on whether this stage could run at all,
    // which is exactly what spec-0023 downgraded to telemetry: the die-retry stage
    // asks "is dying safe here", not "can this bot win". On the-drowned-bell run
    // six two vindicators killed the bot on the way in, `dieRetryAt` threw before
    // scripting death 1, and the stage reported 0/2 with no windows at all.
    //
    // Every segment where the bot must SURVIVE to make a measurement is assisted;
    // the scripted death itself deliberately is not (see below). Each window is
    // opened, logged and closed on its own, so the artifact names exactly when the
    // bot was protected — spec-0023 §3 asks for disclosure, not for one window.
    await this.withAssist(enc, "die-retry: approach into melee range", () =>
      this.walkTo(step.pos, 3, `die-retry approach ${step.wave}`, step.sneak),
    );
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
      //
      // Assisted, for the same reason the approach is: the point of the trade is
      // to put the wave in its mid-fight state before a SCRIPTED death, and a bot
      // the wave kills mid-trade takes an unscripted one instead — a death at
      // roughly the right moment, but not the one this trial asked for.
      if (phase === "mid-fight") {
        await this.withAssist(enc, "die-retry: trading blows before the scripted death", () =>
          this.tradeBlows(step),
        );
        // ...and if the trade ended in a real death anyway, clear it here rather
        // than script a second one on top of it. Without this the `deathSeq` read
        // below already carries the accidental death, so the trial would wait for
        // a death that has to happen AGAIN — crediting the loop to a life the
        // harness never opened (the first-contact/mid-fight race).
        if (this.death) {
          process.stderr.write(
            `[die-retry] the wave killed the bot during the trade — recovering before ` +
              `taking the scripted death, so the death this trial records is the one it asked for\n`,
          );
          await this.respawnAndRearm();
          await this.withAssist(enc, "die-retry: re-approach after an unscripted death", () =>
            this.walkTo(step.pos, 3, `die-retry re-approach ${step.wave}`, step.sneak),
          );
        }
      }
      const before = new Set(this.completedObjectives.keys());
      // BRAND the mobs this life fought. A re-seat must replace every
      // one of them; a mob still wearing the brand in the next life IS the chipped
      // survivor a faithful re-seat forbids. The stamp rides the wave's
      // own tag, so it can only ever land on this wave — the mistake the previous
      // id-based baseline made, which branded whatever the client happened to be
      // tracking.
      this.brandWave(enc);
      process.stderr.write(
        `[die-retry] ${step.wave} death ${attempt}/${phases.length} (${phase})\n`,
      );
      // The record exists from the moment the harness commits to dying. Everything
      // below MUTATES it, so however the run ends the artifact still says a death
      // was taken here and what was (and was not) learned from it.
      const trial = openTrial(enc, attempt, phase);
      this.trials.push(trial);
      try {
        // A cutscene cannot be allowed to eat this death. The encounter's own
        // objective completion may start one, and a cutscene's first act is
        // `gamemode spectator @a` — so a trade that finished the wave hands the
        // next scripted death an invulnerable body, `/damage` does nothing, and
        // the stage used to report that as a missing op.
        await this.awaitControlForScriptedDeath(step, enc);
        const seq = this.deathSeq;
        // What the bot carries INTO the death — the baseline `keep_inventory` is
        // judged against on the way out.
        this.itemsBeforeDeath = bot.inventory.items().length;
        const chatFrom = this.recentChat.length;
        bot.chat(scriptedDeathCommand());
        if (!(await this.awaitDeathAfter(seq, RESPAWN_TIMEOUT_MS))) {
          // No death followed, so the stage proves nothing and the trial fails —
          // but it fails saying what it SAW. The bot is opped and receives the
          // server's own answer on the chat stream, and its gamemode decides
          // whether the command could ever have worked.
          trial.abortedWith = scriptedDeathRefusal(
            scriptedDeathCommand(),
            RESPAWN_TIMEOUT_MS,
            this.gameModeNow(),
            this.recentChat.slice(chatFrom),
          );
          throw new Error(`die-retry: ${trial.abortedWith}`);
        }
        trial.cause = this.death?.likelyCause;
        const respawn = await this.respawnAndRearm();
        trial.respawnPos = respawn.pos;
        trial.kitKept = respawn.kitKept;
        trial.atCheckpoint = respawnedAtCheckpoint(trial.respawnPos ?? [0, 0, 0], enc.checkpoint);
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt}: respawned at ` +
            `${trial.respawnPos ? trial.respawnPos.join(",") : "an unknown position"}` +
            `${trial.atCheckpoint ? "" : ` — NOT the governing checkpoint ${enc.checkpoint?.join(",") ?? "(none)"}`}\n`,
        );
        // The walk back ends INSIDE the re-seated wave, and the probe then stands
        // there for the whole settle — so both are assisted, for the same reason
        // the approach is. Whether the ROUTE is walkable is the measurement; a bot
        // cut down on the last block would answer "no" for a reason that has
        // nothing to do with the route.
        await this.withAssist(enc, "die-retry: walk back and re-engage probe", async () => {
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
          trial.objectiveComplete = this.completedObjectives.has(enc.objective);
          // Two observations, one verdict (see RetryOutcome). A wave mob standing
          // here again means the fight is retriable. Nothing left to fight is only
          // a failure if the encounter's objective is ALSO unfinished — then the
          // party can neither complete it nor re-fight it, which is a soft lock.
          // A wave already beaten before the death is a won fight staying won.
          //
          // Observed ONLY when the bot got back. The probe reads the
          // entities the CLIENT tracks, so a bot standing 150 blocks from the fight
          // is not observing the encounter at all — it is observing wherever it is
          // stuck. Reporting that as `re_engaged` produced the run-five artifact in
          // which one trial said "the route back is not walkable" and "the fight
          // re-engaged" at once. A trial that never returned leaves `re_engaged`
          // false, `reengage` null and its outcome `unproven`: not looked at is not
          // the same fact as looked at and empty, and neither is a pass.
          if (trial.returned) {
            const obs = await this.awaitReengage(enc);
            trial.reengage = obs;
            trial.reEngaged = obs.present > 0;
            trial.outcome = retryOutcome(trial.reEngaged, trial.objectiveComplete);
            process.stderr.write(
              `[die-retry] ${step.wave} death ${attempt}: ${obs.present}/${obs.declared} wave mob(s) ` +
                `after ${obs.settleMs}ms` +
                `${obs.nearest !== undefined ? `, ${obs.nearest.toFixed(1)}–${obs.farthest!.toFixed(1)} blocks from the anchor` : ""}` +
                `${obs.carriedOver > 0 ? `, ${obs.carriedOver} carried over from a previous life` : ""}` +
                `${obs.healthReadable > 0 ? `, ${obs.damaged}/${obs.healthReadable} damaged` : ""}\n`,
            );
          } else {
            process.stderr.write(
              `[die-retry] ${step.wave} death ${attempt}: the bot never got back to the ` +
                `encounter, so re-engagement was NOT observed (outcome stays \`unproven\`)\n`,
            );
          }
        });
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
      } finally {
        // Clear the brand however the trial ended, so the NEXT death brands a
        // clean slate and a stale stamp can never be read as a survivor. In the
        // `finally` because an abandoned trial leaves mobs standing too.
        this.unbrandWave(enc);
      }
    }
  }

  /**
   * The gamemode the client currently believes it is in, as a plain string.
   *
   * Widened deliberately: mineflayer's own type for `bot.game.gameMode` lists
   * `survival | creative | spectator` and omits `adventure`, which is the mode every
   * delve actually runs in. Comparing against the mode a delve uses is not an
   * unintentional comparison; the library's enumeration is short.
   */
  private gameModeNow(): string | undefined {
    return this.bot?.game?.gameMode;
  }

  /**
   * Hold until the bot is a body a scripted death can reach — out of the spectator
   * a cutscene put it in.
   *
   * The bound is the campaign's own number, never one invented here: a step whose
   * completion fires a `Cutscene` carries `cutscene_seconds` in
   * `critical-path.json`, and a `kill` step carries it exactly as a walking step
   * does. The sequencer already waits it out AFTER a step; nothing waited it out
   * inside one, which is where the die-retry stage lives — the general mechanism
   * was there and its binding did not reach this caller. The grace on top is the
   * same {@link awaitCutscene} uses.
   *
   * Bounded, and it never fails: a window that outlasts what the build declared is
   * a finding, and the finding is the refusal {@link scriptedDeathRefusal} writes
   * when the death then does not land — which says the gamemode it saw.
   */
  private async awaitControlForScriptedDeath(step: KillStep, enc: Encounter): Promise<void> {
    this.requireBot();
    if (this.gameModeNow() === CONTROLLED_GAMEMODE) return;
    const declaredMs = (step.cutsceneSeconds ?? 0) * 1000;
    const budget = declaredMs + this.cutsceneGraceMs;
    const started = Date.now();
    const deadline = started + budget;
    process.stderr.write(
      `[die-retry] ${enc.wave}: the bot is in \`${this.gameModeNow() ?? "?"}\`, not ` +
        `\`${CONTROLLED_GAMEMODE}\` — waiting out the cutscene before scripting a death ` +
        `(${step.cutsceneSeconds ?? 0}s declared + ${this.cutsceneGraceMs}ms grace)\n`,
    );
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      if (this.gameModeNow() === CONTROLLED_GAMEMODE) {
        process.stderr.write(
          `[die-retry] ${enc.wave}: control returned after ${Date.now() - started}ms\n`,
        );
        return;
      }
      await delay(CUTSCENE_POLL_MS);
    }
    process.stderr.write(
      `[die-retry] ${enc.wave}: still \`${this.gameModeNow() ?? "?"}\` after ${budget}ms — ` +
        `scripting the death anyway, so the refusal below says what was seen rather than ` +
        `this wait swallowing it\n`,
    );
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
   * content killing the bot (the-drowned-bell round 3).
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
   * Ask the server what is standing at this encounter — BY TAG.
   *
   * The old probe answered by silhouette: every entity the client tracked, no
   * distance filter, anything taller than half a block. That set is not the wave.
   * On the drowned bell it swept in two ambush husks 57 blocks away at the cistern
   * and whichever neighbouring wave had just been re-seated, so a 2-mob wave read
   * as 4 standing — and because those bystanders were alive on both sides of a
   * scripted death, they were reported as survivors the re-seat had failed to
   * remove. The re-seat was innocent; the ruler was wrong.
   *
   * Only the server can see the wave tag, so the compiler emits the census and
   * this reads it: counts of standing / branded / damaged, plus one line per mob
   * with its position and health. Everything the fidelity verdict consumes is the
   * server's own answer about entities carrying `dw_wave_<id>`, and nothing else
   * can enter it.
   *
   * Returns `undefined` if no answer arrives — never a zero, which would read as
   * "the wave is gone".
   */
  private async census(enc: Encounter): Promise<WaveCensus | undefined> {
    const bot = this.requireBot();
    const before = this.censusSeq;
    bot.chat(`/function ${enc.census.census}`);
    const deadline = Date.now() + CENSUS_TIMEOUT_MS;
    for (;;) {
      const sum = this.censusSummary;
      if (sum && sum.seq > before && sum.wave === enc.wave) {
        return { summary: sum, mobs: this.censusMobs.get(sum.seq) ?? [] };
      }
      if (Date.now() >= deadline) return undefined;
      await delay(SCORE_POLL_MS);
    }
  }

  /**
   * How many of the wave stand? Asks the SERVER, by tag, and records the answer.
   *
   * This is the kill step's terminal condition and the source of its attribution,
   * so everything the step decides about the fight comes through here.
   *
   * Every OTHER test in `fightWave` is a guess made from shapes — "a mob the bot
   * hit winked out near the anchor", "everything it engaged is down and nothing
   * hostile is close". None of them can tell a wave mob from any other mob,
   * because the client cannot see the wave tag. On the drowned bell that cost the
   * campaign a whole round: at the belfry the bot killed one of
   * `ambush/the-rafters`' husks, counted it as the Bellkeeper (`confirmed kill:
   * husk#232 (1/1)`), and walked away from a wither skeleton that was still very
   * much alive. `obj/the-keeper` therefore never completed, `quest/the-keeper`
   * never completed, `quest/ring-it-home` was never armed — and the next step's
   * `interact` click was adjudicated against an unarmed quest and spent. The
   * click was the SYMPTOM; this was the cause.
   *
   * The guesses still DRIVE the fight — the bot can only swing at what it can see
   * — and they still prompt a census. What they no longer do is end the step on
   * their own, and neither does the bot's tally of confirmed kills: a body that
   * died where the proximity rule cannot attribute it, or a scripted death that
   * re-seated the wave under that tally, both leave it unable to reach the wave's
   * declared size for the rest of the step.
   *
   * `undefined` when there is nothing to ask (a wave outside the combat plan) or
   * when the census did not answer — never a zero, which would read as "the wave
   * is gone" on a broken probe. The watch is not fed in that case, so a probe that
   * never answers can never clear a fight.
   */
  private async pollWaveCensus(
    step: KillStep,
    enc: Encounter | undefined,
    watch: WaveCensusWatch,
  ): Promise<number | undefined> {
    if (!enc) return undefined;
    const census = await this.census(enc);
    if (!census) {
      process.stderr.write(
        `[kill ${step.wave}] the wave census did not answer; falling back to what the client ` +
          `can see\n`,
      );
      return undefined;
    }
    observeCensus(watch, {
      present: census.summary.present,
      credited: census.summary.credited,
    });
    // Who felled this cohort. `step.count` is the seating the compiler declared
    // and `spawn_<wave>` wrote; the other two are the server's own answer.
    this.waveAttributions.set(
      step.wave,
      waveAttribution(step.count, census.summary.present, census.summary.credited),
    );
    return census.summary.present;
  }

  /** Stamp this life's wave mobs so the next census can name the survivors. */
  private brandWave(enc: Encounter): void {
    this.requireBot().chat(`/function ${enc.census.brand}`);
  }

  /** Clear the stamp, so the next trial brands a clean slate. */
  private unbrandWave(enc: Encounter): void {
    this.requireBot().chat(`/function ${enc.census.unbrand}`);
  }

  /**
   * Buffer a census line. Mob lines arrive before the summary that closes them
   * (one atomic function call, in emission order), so by the time a summary is
   * observed its mobs are already collected under the same sequence number.
   */
  private observeCensus(message: string): void {
    const mob = parseCensusMob(message);
    if (mob) {
      if (mob.campaignId !== this.campaignId) return;
      const at = this.censusMobs.get(mob.seq) ?? [];
      at.push(mob);
      this.censusMobs.set(mob.seq, at);
      // Bounded: only the newest few censuses can still be asked about.
      while (this.censusMobs.size > CENSUS_HISTORY) {
        const oldest = Math.min(...this.censusMobs.keys());
        this.censusMobs.delete(oldest);
      }
      return;
    }
    const sum = parseCensusSummary(message);
    if (!sum || sum.campaignId !== this.campaignId) return;
    this.censusSummary = sum;
    this.censusSeq = Math.max(this.censusSeq, sum.seq);
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
  private async awaitReengage(enc: Encounter): Promise<ReengageObservation> {
    const started = Date.now();
    const deadline = started + REENGAGE_SETTLE_MS;
    let census = await this.census(enc);
    for (;;) {
      // Enough is standing to answer every question this observation feeds.
      if (census && census.summary.present >= enc.count) break;
      if (Date.now() >= deadline) break;
      await delay(REACH_POLL_MS);
      census = (await this.census(enc)) ?? census;
    }
    if (!census) {
      // No census came back at all. That is a broken probe, not an empty room, and
      // the run must say so rather than report a `stranded` the delve never caused.
      throw new Error(
        `die-retry: the wave census \`${enc.census.census}\` never answered within ` +
          `${CENSUS_TIMEOUT_MS}ms — the bot must be opped to call it`,
      );
    }
    return observationOf(census, enc.count, enc.pos, Date.now() - started);
  }

  /** Melee whatever wave mob is closest for a moment, so the next scripted death
   * lands mid-fight rather than at first contact. Best effort by design — if
   * nothing is in reach there is nothing to trade with, and the trial still runs. */
  private async tradeBlows(step: KillStep): Promise<void> {
    const bot = this.requireBot();
    const deadline = Date.now() + MID_FIGHT_MS;
    while (Date.now() < deadline && !this.death) {
      const cast = this.requireNonCombatants();
      const mob = bot.nearestEntity((e) => isWaveMob(e, bot.entity, cast));
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

  /**
   * Wait out a death, note WHERE the bot came back, and ready it to fight again
   * **without moving it**.
   *
   * The re-arm does NOT replay `select-class`. The premise that "a respawn drops
   * class state" is false, and the replay is destructive (the-drowned-bell run
   * five):
   *
   *   * false, because the compiler seals `gamerule keep_inventory true` in every
   *     build — the kit survives the death — and class state lives in scoreboard
   *     values and player tags, which survive it too;
   *   * destructive, because `class_apply_<class>` ends in
   *     `teleport @s <campaign entry point>` and the `dw.class` trigger was, at the
   *     time, re-enabled for every player on every tick. Chatting it again therefore
   *     teleported the bot off the checkpoint it had just respawned on, back to the
   *     start of the delve — 150 blocks and eight levels away on the bell — and the
   *     "walk back to the encounter" leg then measured a route no dying player ever
   *     walks. Every trial recorded a truthful `respawn_pos` at the bonfire and then
   *     immediately made it a lie.
   *
   * The compiler now seals that warp shut: the class trigger is armed only
   * for a player who has not classed, so a replay would fail rather than teleport.
   * The rule here is unchanged and does not lean on it — a harness that re-classes
   * is stating something false about the run whether or not the pack lets it.
   *
   * So: measure the position, then re-equip what the player kept. Nothing here may
   * move the bot — the respawn point IS the thing under test.
   */
  private async respawnAndRearm(): Promise<{ pos: Vec3Tuple | undefined; kitKept: boolean }> {
    const bot = this.requireBot();
    await this.recoverFromDeath();
    const p = bot.entity?.position;
    const pos: Vec3Tuple | undefined =
      p === undefined ? undefined : [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)];
    const kitKept = await this.rearmAfterRespawn();
    return { pos, kitKept };
  }

  /**
   * Ready the bot to fight again after a respawn, without teleporting it.
   *
   * `keep_inventory` means the kit is still in the bag; what a respawn does clear
   * is the *equipped* state a client tracks, so the loadout is put back on. Returns
   * whether the kit survived: a bag that carried items into the death and is still
   * empty {@link KIT_SETTLE_MS} after the respawn lost them, which breaks the retry
   * loop for a human player exactly as it breaks it for the bot.
   */
  async rearmAfterRespawn(): Promise<boolean> {
    const kept = await this.awaitKeptKit();
    if (!kept) {
      process.stderr.write(
        `[death] the bot came back EMPTY-HANDED: it carried items into the death and the ` +
          `bag is still empty ${KIT_SETTLE_MS}ms after the respawn. The delve seals ` +
          `\`gamerule keep_inventory true\`, so a lost kit means that seal is not in force\n`,
      );
    }
    await this.equipLoadout();
    return kept;
  }

  /**
   * Wait for the kept inventory to arrive, bounded. `true` the moment an item is
   * visible (and immediately for a bot that carried nothing into the death — there
   * is nothing to keep, so nothing was lost).
   */
  private async awaitKeptKit(): Promise<boolean> {
    const bot = this.requireBot();
    if (this.itemsBeforeDeath === 0) return true;
    const deadline = Date.now() + KIT_SETTLE_MS;
    for (;;) {
      if (bot.inventory.items().length > 0) return true;
      if (Date.now() >= deadline) return false;
      await delay(SPAWN_POLL_MS);
    }
  }

  /**
   * Rest at a bonfire — the player loop every later proof depends on.
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
    await this.walkTo(step.pos, REST_RANGE, `bonfire ${step.anchor}`, step.sneak);
    // The one step in the tree with a REAL right-click, so acquisition and
    // actuation are the same act here: the ray picks the fire, the bot looks
    // where the ray went, and only then does it click. Selecting the nearest
    // affordance to a coordinate — what this used to do — cannot tell a fire from
    // whatever is standing in front of it.
    const acquired = this.requireCrosshair(step.pos, `bonfire ${step.anchor}`, REST_RANGE);
    const fire = acquired ? bot.entities[acquired.target.id] : undefined;
    if (!acquired || !fire) {
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
    const here = bot.entity.position;
    await bot.lookAt(
      here.offset(acquired.aim.x - here.x, acquired.aim.y - here.y, acquired.aim.z - here.z),
      true,
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

  /**
   * Every ray-pickable body the client currently tracks near `pos`, as crosshair
   * geometry. Anything with a hitbox counts — a body occludes whether or not it
   * is itself clickable, which is the whole reason the owner's two crew NPCs
   * blocked each other.
   */
  private hitboxesNear(pos: Vec3Tuple, radius: number): Hitbox[] {
    const bot = this.requireBot();
    const out: Hitbox[] = [];
    for (const e of Object.values(bot.entities)) {
      if (!e?.position || e.id === bot.entity?.id) continue;
      const dims = hitboxDims(e.name ?? "", e.width, e.height);
      if (!dims) continue;
      const d = Math.sqrt(
        (e.position.x - (pos[0] + 0.5)) ** 2 +
          (e.position.y - pos[1]) ** 2 +
          (e.position.z - (pos[2] + 0.5)) ** 2,
      );
      if (d > radius) continue;
      out.push({
        id: e.id,
        name: e.name ?? "unknown",
        label: displayNameOf(e),
        position: { x: e.position.x, y: e.position.y, z: e.position.z },
        width: dims.width,
        height: dims.height,
      });
    }
    return out;
  }

  /**
   * The body a step means to click at `pos`: the `minecraft:interaction` box the
   * compiler summoned there.
   *
   * Every clickable thing in a delve is one of those — an NPC's dialogue hitbox
   * as much as an objective's affordance — so this is the single acquisition rule
   * for all three interaction steps. Selection is still by proximity to the
   * scripted cell (a client cannot read the entity tag the compiler uses), but
   * proximity now only proposes the target; the RAY decides whether it is
   * reachable.
   */
  private interactionTargetAt(pos: Vec3Tuple, candidates: readonly Hitbox[]): Hitbox | undefined {
    let best: Hitbox | undefined;
    let bestDist = AFFORDANCE_RADIUS;
    for (const c of candidates) {
      if (c.name !== "interaction") continue;
      const d = Math.sqrt(
        (c.position.x - (pos[0] + 0.5)) ** 2 +
          (c.position.y - pos[1]) ** 2 +
          (c.position.z - (pos[2] + 0.5)) ** 2,
      );
      if (d <= bestDist) {
        best = c;
        bestDist = d;
      }
    }
    return best;
  }

  /**
   * Whether a player could stand with their feet in `cell`: solid support below,
   * two cells of clear air, and no entity body already occupying the column.
   *
   * Block-shape based (`boundingBox`), like {@link gateOpen}, so it stays correct
   * for whatever the campaign built with. A cell whose chunk is not loaded reads
   * as NOT standable — the conservative direction here, since inventing a stance
   * would let a real occlusion pass.
   */
  private stanceStandable(cell: Vec3Tuple, bodies: readonly Hitbox[]): boolean {
    const bot = this.requireBot();
    const p = bot.entity.position;
    const at = (dy: number) =>
      bot.blockAt(p.offset(cell[0] - p.x, cell[1] + dy - p.y, cell[2] - p.z));
    const support = at(-1);
    const feet = at(0);
    const head = at(1);
    if (!support || !feet || !head) return false;
    if (support.boundingBox === "empty") return false;
    if (feet.boundingBox !== "empty" || head.boundingBox !== "empty") return false;
    // A player is 0.6 wide and cannot share a column with another body.
    const cx = cell[0] + 0.5;
    const cz = cell[2] + 0.5;
    return !bodies.some((b) => {
      const half = (b.width + PLAYER_HITBOX_WIDTH) / 2;
      return (
        Math.abs(b.position.x - cx) < half &&
        Math.abs(b.position.z - cz) < half &&
        b.position.y < cell[1] + PLAYER_HITBOX_HEIGHT &&
        cell[1] < b.position.y + b.height
      );
    });
  }

  /**
   * Every eye position this step allows, arrival stance first.
   *
   * The step's walk goal is `GoalNear(pos, range)`, so any standable cell inside
   * that disc is a place the player may legally be standing when they click —
   * which is why a failure here means "unclickable from ANYWHERE the step
   * permits", not "unclickable from where the bot happened to stop".
   */
  private stancesAround(pos: Vec3Tuple, range: number, bodies: readonly Hitbox[]): Vec3Like[] {
    const bot = this.requireBot();
    const eye = bot.entity.position;
    const out: Vec3Like[] = [{ x: eye.x, y: eye.y + PLAYER_EYE_HEIGHT, z: eye.z }];
    for (let dy = -1; dy <= 1; dy += 1) {
      for (let dx = -range; dx <= range; dx += 1) {
        for (let dz = -range; dz <= range; dz += 1) {
          const cell: Vec3Tuple = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
          if (!this.stanceStandable(cell, bodies)) continue;
          out.push({
            x: cell[0] + 0.5,
            y: cell[1] + PLAYER_EYE_HEIGHT,
            z: cell[2] + 0.5,
          });
        }
      }
    }
    return out;
  }

  /**
   * Prove a player could put the crosshair on this step's target before the step
   * acts on it — the assertion the island's terminal finding needed.
   *
   * Throws, naming both bodies, when the target is unpickable from every stance
   * the step allows. Returns the acquired target (and the aim point) when it is
   * reachable, so a caller with a real click to make can look at it first.
   *
   * When no `interaction` body is tracked at `pos` at all, this reports a finding
   * and returns `undefined` rather than failing: absence is "the client has not
   * been told", not "the player cannot click", and inventing a verdict from
   * missing data is the failure mode this whole change exists to end.
   */
  private requireCrosshair(
    pos: Vec3Tuple,
    what: string,
    range: number,
  ): { target: Hitbox; aim: Vec3Like } | undefined {
    const bodies = this.hitboxesNear(pos, INTERACTION_REACH + CROSSHAIR_SEARCH_MARGIN);
    const target = this.interactionTargetAt(pos, bodies);
    if (!target) {
      process.stderr.write(
        `[crosshair] ${what}: no \`interaction\` body tracked within ${AFFORDANCE_RADIUS} ` +
          `blocks of [${pos.join(", ")}] — acquisition unproven for this step\n`,
      );
      return undefined;
    }
    const others = bodies.filter((b) => b.id !== target.id);
    const stances = this.stancesAround(pos, range, bodies);
    const verdict = acquireFromStances(stances, target, others);
    if (!verdict.ok) {
      throw new Error(occlusionFailure(what, target, verdict.blockers, verdict.triedStances));
    }
    if (verdict.clearStances < verdict.triedStances) {
      process.stderr.write(
        `[crosshair] ${what}: acquired from ${verdict.clearStances} of ` +
          `${verdict.triedStances} allowed stances — the target is clickable, but not from ` +
          `every place the party may be standing\n`,
      );
    }
    return { target, aim: verdict.aim };
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

  /** Adopt the rest steps the exported critical path carries, with
   * their EXPORTED indices — the coordinate system the precondition compares in. */
  useRestSteps(rests: readonly PerformedRest[]): void {
    this.restSteps = rests;
  }

  /** spec-0029 name-preference binding counters (see {@link NamePreference}). */
  private namePreferenceDecisions = 0;
  private namePreferenceWithName = 0;
  private namePreferenceCandidates = 0;
  private namePreferenceNamedCandidates = 0;

  /**
   * The measured name-preference binding for the run report (spec-0029). Always
   * reported, including the all-zero shape — a preference nobody exercised is a
   * stated zero binding, never a silent absence.
   */
  namePreference(): NamePreference {
    return {
      decisions: this.namePreferenceDecisions,
      withUsableName: this.namePreferenceWithName,
      candidates: this.namePreferenceCandidates,
      namedCandidates: this.namePreferenceNamedCandidates,
    };
  }

  /** Rests this run performed, and the bonfires among them. For the run report. */
  performedRests(): readonly PerformedRest[] {
    return this.restSteps.filter((r) => this.restedBonfires.has(r.bonfire));
  }

  /** Encounters whose scripted deaths were skipped for want of an ARMED
   * checkpoint — the run's own gap, so these red the stage. */
  dieRetryPreconditionFindings(): readonly string[] {
    return this.preconditionFindings;
  }

  /** Encounters whose scripted deaths were skipped because the campaign fires no
   * governing checkpoint before them. Advisory: the retry loop there went
   * unproven, and whether that staging is acceptable is the compiler's call, not
   * this stage's. Reported, never graded — and never silent. */
  dieRetryPreconditionAdvisories(): readonly string[] {
    return this.preconditionAdvisories;
  }

  /** The waves those findings name. Coverage stays silent about them: the
   * precondition already says why they are unproven, and "never reached this
   * encounter" would be plainly untrue — the bot stood in the room and declined. */
  dieRetryPreconditionWaves(): ReadonlySet<string> {
    return this.preconditionWaves;
  }

  /**
   * Collect items from the chest at the anchor: go there, open it, withdraw all.
   *
   * A **drop-gated** collect (v0.9 `dropped_by`) has no chest to open — the
   * compiler places none, because the item exists only after the fight. The bot
   * walks the ground the wave died on and lets vanilla pickup do the rest; the
   * proof is the same one every collect uses, the objective's own marker.
   */
  async collect(step: CollectStep): Promise<void> {
    const bot = this.requireBot();
    if (step.droppedBy !== undefined) {
      await this.walkTo(step.pos, 1, `drop of ${step.item}`, step.sneak, {
        objective: step.objective,
        transport: step.transport,
      });
      await this.requireObjective(step.objective, `collect ${step.item}`);
      return;
    }
    await this.walkTo(step.pos, 2, `chest ${step.item}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
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
   * `requires_item` against the MAINHAND, which is why the hand
   * is loaded first. See {@link presentAndTrigger}.
   */
  async interact(step: InteractStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, INTERACT_RANGE, `interact ${step.anchor}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
    // Same proof as `talkTo`: the affordance the party has to right-click must be
    // what a crosshair actually reaches, not merely what is nearest the cell.
    this.requireCrosshair(step.pos, `interact ${step.anchor}`, INTERACT_RANGE);
    this.armTrigger(step.command);
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
   * Three deterministic phases, each bounded and death-aware so nothing
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
      () => this.atTransportDest(dest),
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
   * Wait once, at spawn, for `bot.entities` to stop changing shape before anything
   * reads it (2026-08-06 island triage — see entity-settle.ts). Polls the
   * non-player entity count until it has held steady for
   * {@link hasSettled}'s stability window, or gives up after
   * {@link entitySettleTimeoutMs} and proceeds regardless — a build that
   * legitimately spawns nothing near the bot must not hang the run behind this
   * wait, and every caller downstream (`requireCrosshair` foremost) already
   * reports an empty tracker honestly rather than inventing a verdict from it.
   */
  async awaitEntitySettle(): Promise<void> {
    const bot = this.requireBot();
    const history: number[] = [];
    const settled = await this.waitFor(
      () => {
        const count = Object.values(bot.entities).filter(
          (e) => e && e.id !== bot.entity?.id && e.type !== "player",
        ).length;
        history.push(count);
        return hasSettled(history);
      },
      this.entitySettleTimeoutMs,
      ENTITY_SETTLE_POLL_MS,
    );
    const last = history[history.length - 1] ?? 0;
    if (settled) {
      process.stderr.write(
        `[entity-settle] tracker settled at ${last} non-player entit${last === 1 ? "y" : "ies"} ` +
          `after ${history.length} poll(s)\n`,
      );
    } else {
      process.stderr.write(
        `[entity-settle] gave up after ${this.entitySettleTimeoutMs}ms waiting for the entity ` +
          `tracker to stop changing shape (last count: ${last}) — proceeding; a step that finds ` +
          `nothing tracked still reports that honestly rather than failing on it\n`,
      );
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
    // (assertEndgameNotReached), so reaching here means it is either due now or —
    // when the path exports a scheduled-ending tail (`ending_tail_ticks`: the-wake
    // fires `campaign-complete` 250t into its closing `sequence`) — due within
    // that tail. The window covers whichever is longer.
    const windowMs = completionWindowMs(step.endingTailTicks);
    const deadline = Date.now() + windowMs;
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
      `campaign not complete after ${windowMs}ms: no ` +
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
 * An entity's custom name as plain text, or `undefined` when it has none.
 *
 * mineflayer surfaces a custom name in several shapes across versions — a plain
 * string, a chat component with `toString`, or `{ text }` — so this reads all of
 * them and gives up quietly rather than throwing. Used only to PREFER the right
 * body among candidates of the same entity type; identity never rests on
 * it, because a client cannot read the entity tag the compiler actually uses.
 *
 * **i18n v2 (spec-0029).** An authored name now ships as
 * `{"translate": "<l10n key>", "fallback": "<English source>"}`, so the `text`
 * field is gone. `fallback` is read explicitly and FIRST among the component
 * shapes: it is by construction the English source the campaign document holds,
 * which is exactly the string the plan's `actors[].name` carries — so the
 * preference heuristic keeps matching, rather than depending on whether the
 * installed prismarine-chat resolves an unknown translate key to its fallback or
 * to the raw key. How often it actually binds is MEASURED, not assumed:
 * {@link NamePreference} counts every candidate-preference decision and how many
 * had a usable name, and the run report prints both.
 */
export function displayNameOf(e: unknown): string | undefined {
  const ent = e as { displayName?: unknown; customName?: unknown };
  for (const raw of [ent.customName, ent.displayName]) {
    if (typeof raw === "string" && raw.length > 0) return raw;
    if (raw && typeof raw === "object") {
      const o = raw as { text?: unknown; fallback?: unknown; toString?: () => string };
      if (typeof o.fallback === "string" && o.fallback.length > 0) return o.fallback;
      if (typeof o.text === "string" && o.text.length > 0) return o.text;
      const s = typeof o.toString === "function" ? o.toString() : "";
      if (s && s !== "[object Object]") return s;
    }
  }
  return undefined;
}

/**
 * How often the same-type candidate preference actually had a name to
 * prefer by — the binding count spec-0029 requires the bot run to state rather
 * than assume.
 *
 * `decisions` counts every time the bot chose among candidate bodies for a named
 * actor; `withUsableName` counts the ones where at least one candidate carried a
 * readable custom name. A run whose `decisions` is 0 examined nothing and is
 * `unbound` — a finding, not a pass (CLAUDE.md, playtest-methodology.md rule 1).
 * A run with decisions but `withUsableName: 0` is the specific regression
 * spec-0029 asks to watch for: translate components rendering as keys the
 * heuristic cannot match.
 */
export interface NamePreference {
  readonly decisions: number;
  readonly withUsableName: number;
  readonly candidates: number;
  readonly namedCandidates: number;
}

/**
 * Vanilla shapes that are never a combat target, whatever campaign is running.
 *
 * Everything here is a fact about MINECRAFT and about the bot's own situation: a
 * player is not a monster, a dropped item is not a body, a display entity has no
 * health. No campaign can make any of them a fight, so no campaign has to say so.
 *
 * **What is deliberately NOT here**: which bodies THIS delve stages as NPCs. That
 * used to be `"mannequin"` and `"villager"`, written down beside the vanilla
 * shapes as though it were the same kind of fact. It is not — it is a statement
 * about what the compiler summons an NPC as, and an author who bodies a
 * quest-giver as a zombie gets a quest-giver the bot beats to death. The delve now
 * states its own cast in `critical-path.json`'s `non_combatants`, which
 * {@link isWaveMob} takes as an argument; see {@link Executor.useNonCombatants}.
 */
const NON_WAVE_ENTITIES = new Set<string>([
  "player",
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
 * True if `e` is something the bot could swing at: not the bot, not a vanilla
 * non-body, not one of the kinds THIS delve stages as an NPC, and tall enough to
 * be a living mob. Classified by name (reliable across mineflayer versions)
 * rather than `type`/`kind`, which vary.
 *
 * `nonCombatants` is the delve's own cast statement, read off
 * `critical-path.json` — required, never defaulted. Passing an empty set is a
 * legitimate answer (a campaign with no NPCs states exactly that, and says so in
 * its binding count); omitting the argument is not possible, which is the point.
 *
 * **This is a TARGETING predicate, never a measurement one**. The bot
 * can only attack what its client can see, so picking a swing target by shape is
 * the only thing it could do — but ANSWERING a question about the wave that way
 * is how the drowned bell's ambush husks were reported as wave mobs a re-seat had
 * failed to remove. Every count that reaches a verdict or the run report
 * now comes from the compiler's tag census instead; nothing here may be used to
 * decide what is standing at an encounter.
 */
export function isWaveMob(e: unknown, self: unknown, nonCombatants: ReadonlySet<string>): boolean {
  if (!e || e === self) return false;
  const ent = e as { name?: string; height?: number };
  const name = ent.name ?? "";
  if (name === "" || NON_WAVE_ENTITIES.has(name) || nonCombatants.has(name)) return false;
  return (ent.height ?? 0) >= 0.5;
}
