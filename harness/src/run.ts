// Harness entrypoint: `node src/run.ts <critical-path.json>`.
// Loads and validates the compiler's critical-path contract (spec-0002), connects
// a mineflayer bot to the server (connection details from the environment — see
// executor.ts), executes the critical path under a hard wall-clock budget, and
// exits 0 on success / 1 on any failure (parse error, ordering violation, a failed
// step, or timeout). No campaign knowledge lives here (spec-0003): everything
// comes from critical-path.json.

import { readFile } from "node:fs/promises";
import nodePath from "node:path";
import { parseCriticalPathJson } from "./critical-path.ts";
import { runSequence, StepExecutionError } from "./sequencer.ts";
import { botConfigFromEnv, MineflayerExecutor } from "./executor.ts";
import { BotDeathError } from "./death.ts";
import { classifyNamedEntityDeaths } from "./teardown.ts";
import {
  branchWaypointsFileFor,
  loadWaypointsForBranchPath,
  loadWaypointsForCriticalPath,
  type Waypoints,
} from "./waypoints.ts";
import {
  actorExercise,
  assistPolicy,
  dieRetryBinding,
  dieRetryCoverageFailures,
  dieRetryFindings,
  loadCombatPlanForCriticalPath,
} from "./combat.ts";
import {
  deathLoopBinding,
  deathLoopBindingFailures,
  lethalTrialFailures,
  loadDeathPlanForCriticalPath,
} from "./death-loop.ts";
import {
  RunReport,
  reportPathFromEnv,
  writeRunReport,
  type ActorReport,
  type BranchOutcome,
  type CrashStage,
  type EncounterReport,
} from "./report.ts";
import { installCrashReporter } from "./crash.ts";
import {
  assertEntryChoicesOnPath,
  branchTierFromEnv,
  drivenBranchFromEnv,
  loadBranchPlanForCriticalPath,
  resolveDrivenBranch,
  selectBranches,
  type PlannedBranch,
} from "./branch-plan.ts";

/**
 * Exit code for a run that failed specifically because the bot died (spec-0008),
 * distinct from the generic failure code (1) used for parse/ordering/navigation
 * faults, so the ladder can tell a lethal-content death from a navigation bug.
 */
const EXIT_BOT_DEATH = 3;

/** True if `err` (or the step failure wrapping it) is a bot death. */
function isBotDeath(err: unknown): boolean {
  if (err instanceof BotDeathError) return true;
  return err instanceof StepExecutionError && err.cause instanceof BotDeathError;
}

/**
 * Whether to retry once after a bot death (spec-0008). Opt-in via
 * DELVEWRIGHT_RETRY_ON_DEATH (`1`/`true`); default fail-fast. For future
 * lethal-delve validation — the safe-route ladder leaves it off.
 */
function retryOnDeathFromEnv(env = process.env): boolean {
  const raw = env["DELVEWRIGHT_RETRY_ON_DEATH"];
  return raw === "1" || raw === "true";
}

/**
 * Hard wall-clock budget for the whole run (spec-0003: 20 min for M1).
 * Override with DELVEWRIGHT_RUN_TIMEOUT_MS. A run exceeding this fails red — a
 * hung bot must not hang CI.
 */
function runTimeoutMs(env = process.env): number {
  const raw = env["DELVEWRIGHT_RUN_TIMEOUT_MS"];
  if (raw === undefined || raw.length === 0) {
    return 20 * 60 * 1000;
  }
  const ms = Number.parseInt(raw, 10);
  if (!Number.isInteger(ms) || ms <= 0) {
    throw new Error(
      `DELVEWRIGHT_RUN_TIMEOUT_MS must be a positive integer, got ${JSON.stringify(raw)}`,
    );
  }
  return ms;
}

/**
 * Whether the die-retry ladder stage runs (spec-0023 §1). ON whenever the build
 * carries a combat plan: it is a REQUIRED stage, not an option — "dying is always
 * safe" is the load-bearing property of a souls delve, and the machine has to
 * prove it. `DELVEWRIGHT_DIE_RETRY=0` skips it for local iteration only, and the
 * run report says so, so a skipped stage can never be mistaken for a passed one.
 */
function dieRetryFromEnv(env = process.env): boolean {
  return env["DELVEWRIGHT_DIE_RETRY"] !== "0";
}

/**
 * Whether the actor floor gate runs. ON whenever the build's combat plan
 * declares a tiered actor. `DELVEWRIGHT_ACTOR_FLOOR=0` skips the engagements for
 * local iteration; the report then records each actor as SKIPPED with that
 * reason, never as measured — the same discipline `DELVEWRIGHT_DIE_RETRY=0`
 * follows, and for the same reason.
 */
function actorFloorFromEnv(env = process.env): boolean {
  return env["DELVEWRIGHT_ACTOR_FLOOR"] !== "0";
}

/**
 * Whether the death-loop stage runs. ON whenever the build ships a
 * death plan — like die-retry it is a REQUIRED stage, not an option: a PackTest
 * fake player is permanently undamageable (measured 2026-08-03 and 2026-08-09),
 * so this tier is the ONLY place a player death can be witnessed at all, and
 * every consequence a lethal volume, an `on_death` and a recovery stake promise
 * is unproven without it. `DELVEWRIGHT_DEATH_LOOP=0` skips it for local
 * iteration, and the report records that it was skipped — never that it passed.
 */
function deathLoopFromEnv(env = process.env): boolean {
  return env["DELVEWRIGHT_DEATH_LOOP"] !== "0";
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`run exceeded wall-clock budget of ${ms}ms`));
    }, ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

/**
 * What the run is doing, for the crash reporter. `startup` until the bot connects;
 * from then on the executor is the authority, because it is the only party that
 * knows whether the current step is inside the die-retry stage.
 */
let crashStage: CrashStage = "startup";
/** The live report, once the campaign id is known. */
let liveReport: RunReport | undefined;
/** The live executor, once it exists — asked for the stage it is inside. */
let liveExecutor: MineflayerExecutor | undefined;

// Armed BEFORE anything can reject. A crash after this point ends the process with
// `harness_crash` in the run report, so a harness fault is never read as a stage
// the delve failed. See crash.ts.
installCrashReporter({
  stage: () => (crashStage === "startup" || crashStage === "connect"
    ? crashStage
    : (liveExecutor?.currentStage() ?? crashStage)),
  report: () => (liveReport ??= new RunReport("unknown", "unknown")),
  reportPath: () => reportPathFromEnv(),
});

async function main(): Promise<number> {
  const pathArg = process.argv[2];
  if (pathArg === undefined || pathArg.length === 0) {
    process.stderr.write("usage: node src/run.ts <path-to-critical-path.json>\n");
    return 1;
  }

  const exported = await readFile(pathArg, "utf8");

  // spec-0025 §3 — the branch tier. A build that declares narrative branches ships
  // `validation/branch-plan.json` (which branches exist, how a player enters each)
  // and one executable path per branch. `DELVEWRIGHT_BRANCHES` says which branches
  // this tier is answerable for; `DELVEWRIGHT_BRANCH` says which one THIS session
  // walks — one per world, because party progress only ever moves forward, so a
  // second branch needs a second world (`validation/branch-runs.sh` is that loop).
  // Neither set → exactly the run this harness has always done.
  const branchPlan = await loadBranchPlanForCriticalPath(pathArg);
  const tier = branchTierFromEnv();
  const selection = branchPlan ? selectBranches(branchPlan, tier) : undefined;
  const drivenId = drivenBranchFromEnv();
  let driven: PlannedBranch | undefined;
  let branchPathFile: string | undefined;
  let text = exported;
  if (drivenId !== undefined) {
    if (branchPlan === undefined || selection === undefined) {
      throw new Error(
        `DELVEWRIGHT_BRANCH=${drivenId} but this build declares no branches ` +
          `(no validation/branch-plan.json beside ${pathArg}) — nothing to drive`,
      );
    }
    driven = resolveDrivenBranch(branchPlan, selection, drivenId);
    branchPathFile = nodePath.join(branchPlan.dir, driven.pathFile!);
    text = await readFile(branchPathFile, "utf8");
    process.stderr.write(
      `branch run: ${driven.id} (flags set ${driven.flagsSet.join(",") || "none"}; ` +
        `unset ${driven.flagsUnset.join(",") || "none"}) via ${branchPathFile}\n`,
    );
  }
  const criticalPath = parseCriticalPathJson(text);

  // The branch's scripted dialogue choices ride INSIDE its path: each `talk-to`
  // step carries the `/trigger` line of the option belonging to this branch (a
  // dialog button is client-rendered and unclickable by a bot, so chatting the
  // line the button runs is the player-legal actuation — spec-0002's amendment).
  // This asserts the path really takes them, so a run cannot report branch
  // coverage while having walked somebody else's storyline.
  const entryCommands = driven ? assertEntryChoicesOnPath(driven, criticalPath.steps) : [];
  if (driven && entryCommands.length > 0) {
    process.stderr.write(`scripted branch choice(s): ${entryCommands.join(" ; ")}\n`);
  }

  // If the compiler's proven waypoint artifact accompanies the
  // path being walked, the executor navigates each walked leg through it
  // (successive nearby goals) so no single distant A* solve strands the bot on a
  // large open cave. Malformed → hard failure.
  //
  // The artifact's legs are consumed in LOCKSTEP with the walked positions of
  // the path being walked, so each path gets ITS OWN artifact: the exported path
  // its `critical-path-waypoints.json`, a driven branch its
  // `branch-waypoints-<slug>.json`, whose legs follow that branch's
  // own step sequence. The critical-path artifact is never replayed on a branch
  // — its legs are position-ordered for a different sequence, and the wrong legs
  // would strand the bot while looking like a content fault.
  let waypoints: Waypoints | undefined;
  let waypointFinding: string | undefined;
  if (driven !== undefined && branchPathFile !== undefined) {
    waypoints = await loadWaypointsForBranchPath(branchPathFile);
    if (waypoints === undefined) {
      // The LOUD fallback (never silent): an un-waypointed branch walk is
      // terrain-flaky where the waypointed one is deterministic — 3 of 4 island
      // branch runs failed on exactly this — so a run that had to walk without
      // the artifact says so on stderr AND in the run report.
      waypointFinding =
        `branch ${driven.id} is walking WITHOUT a per-branch waypoint artifact ` +
        `(${branchWaypointsFileFor(branchPathFile)} is absent): single-goal navigation ` +
        `fallback, which is terrain-flaky on open ground where waypointed navigation is ` +
        `deterministic. Rebuild the delve with a delvec that exports ` +
        `per-branch waypoints; do not trust a strand on this run as a content verdict`;
      process.stderr.write(`[finding] ${waypointFinding}\n`);
    }
  } else {
    // Absent → single-goal navigation (a campaign
    // with no walked leg emits no artifact at all, so absence here is normal).
    waypoints = await loadWaypointsForCriticalPath(pathArg);
  }

  // The path's rest steps, with their EXPORTED indices. The bot
  // performs them as ordinary steps; the die-retry precondition needs to know they
  // exist even when one was not reached, so it is told up front.
  const restSteps = criticalPath.steps.flatMap((s, i) =>
    s.action === "rest"
      ? [{ bonfire: s.bonfire, anchor: s.anchor, pos: s.pos, step: i }]
      : [],
  );

  const config = botConfigFromEnv();
  const budgetMs = runTimeoutMs();
  process.stderr.write(
    `connecting to ${config.host}:${config.port} as ${config.username} ` +
      `(mc ${config.version}, auth ${config.auth}, budget ${budgetMs}ms)\n`,
  );

  const executor = new MineflayerExecutor(config);
  liveExecutor = executor;
  // Scope the completion oracle to this campaign: only markers naming it count
  // (AUDIT-P0). Comes from the contract, never inferred.
  executor.useCampaign(criticalPath.campaignId);
  if (restSteps.length > 0) {
    executor.useRestSteps(restSteps);
    process.stderr.write(
      `${restSteps.length} bonfire rest(s) on the proven path — a fire only ARMS on ` +
        `arrival; the checkpoint moves when the party rests\n`,
    );
  }
  if (waypoints) {
    executor.useWaypoints(waypoints);
    process.stderr.write(
      `using compiler-proven waypoints: ${waypoints.legs.length} walked leg(s)\n`,
    );
  }
  // spec-0023: with the compiler's combat plan, `kill` steps become verified
  // ENCOUNTERS — the die-retry stage proves dying is safe, and the fights run under
  // bounded, labelled combat assist so bot fencing skill never caps how hard a
  // delve is allowed to be. Absent → the pre-spec-0023 run, unchanged.
  // The build's death contract — the lethal volumes the bot may walk
  // into, the wording each promises, the `on_death` consequences and the recovery
  // stake's compile-time placement table. Handed over BEFORE the connect, because
  // it also tells the navigator which cells are impassable: the compiler treats a
  // lethal volume as impassable in every route proof, and a bot that disagrees
  // walks its way home through the hazard it just died in.
  const deathPlan = await loadDeathPlanForCriticalPath(pathArg);
  // An UNBOUND plan does not make the run red — it makes it loud. A campaign that
  // declares a lethal volume and no `on_death` has nothing for this stage to
  // assert, and inventing a content rule that says it must is a design decision,
  // not an inference. What rule 1 requires is that the zero is REPORTED, which is
  // what the finding below does.
  const deathLoop = deathPlan !== undefined && !deathPlan.binding.unbound && deathLoopFromEnv();
  if (deathPlan) {
    executor.useDeathPlan(deathPlan);
    const b = deathPlan.binding;
    process.stderr.write(
      `death plan: ${b.volumes} lethal volume(s), ${b.onDeathEffects} on_death effect(s), ` +
        `${b.stakes} stake(s), ${b.seats} respawn seat(s), ${b.rows} placement row(s); ` +
        `death-loop stage ${deathLoop ? "ON" : "SKIPPED"}\n`,
    );
    if (b.unbound) {
      // playtest-methodology rule 1, at the earliest possible moment: a contract
      // that binds to nothing is REPORTED, never quietly walked.
      process.stderr.write(`death plan: UNBOUND — ${b.reason ?? "no reason given"}\n`);
    }
  }
  const combatPlan = await loadCombatPlanForCriticalPath(pathArg);
  const dieRetry = combatPlan !== undefined && dieRetryFromEnv();
  const actorFloor = actorFloorFromEnv();
  const report = new RunReport(criticalPath.campaignId, combatPlan?.difficulty ?? "unknown");
  // From here a crash writes THIS report — with everything the run had already
  // established in it — rather than the placeholder.
  liveReport = report;
  // Which objectives this run proves — the set that decides which actor fights it
  // can reach at all. Taken from the path being WALKED, so a branch run
  // (spec-0025) measures the actors its own storyline unleashes.
  const pathObjectives = new Set(
    criticalPath.steps.flatMap((s) => ("objective" in s ? [s.objective] : [])),
  );
  executor.usePathObjectives(pathObjectives);
  // Who the bot may never swing at, from the path itself. Handed over before
  // anything can fight: the executor refuses to classify a body without it,
  // because the only alternative is a list of entity names in the harness.
  executor.useNonCombatants(criticalPath.nonCombatants.kinds);
  {
    const nc = criticalPath.nonCombatants;
    process.stderr.write(
      `cast: ${nc.examined} NPC body(ies) examined; never a target: ` +
        `${[...nc.kinds].sort().join(', ') || 'none'}\n`,
    );
    if (nc.unbound) {
      // playtest-methodology rule 1: a census that matched nothing says so,
      // rather than leaving an empty list to read as a pass.
      process.stderr.write(`cast: UNBOUND — ${nc.reason ?? 'no reason given'}\n`);
    }
    for (const a of nc.ambiguous) {
      // The compiler could not exclude this kind without making a fight
      // unwinnable. Nobody may discover that from a corpse.
      process.stderr.write(`cast: AMBIGUOUS \`${a.kind}\` — ${a.why}\n`);
    }
  }
  if (combatPlan) {
    executor.useCombatPlan(combatPlan, dieRetry, actorFloor);
    process.stderr.write(
      `combat plan: ${combatPlan.encounters.length} mandatory encounter(s) at ` +
        `difficulty '${combatPlan.difficulty}'; die-retry ${dieRetry ? "ON" : "SKIPPED"}\n`,
    );
    if (combatPlan.actors.length > 0) {
      const exercisable = combatPlan.actors.filter(
        (a) => actorExercise(a, pathObjectives).kind === "exercise",
      ).length;
      process.stderr.write(
        `combat plan: ${combatPlan.actors.length} tiered actor(s), ${exercisable} reachable on ` +
          `this path; actor floor gate ${actorFloor ? "ON" : "SKIPPED"}\n`,
      );
    }
    if (!combatPlan.floorGate.present) {
      process.stderr.write(
        `combat plan: NO floor-gate ledger — this build predates it; the run cannot tell you ` +
          `which billed fights the gate covers\n`,
      );
    } else if (combatPlan.floorGate.binding?.unbound) {
      // playtest-methodology.md rule 1: a gate that examined zero objects is a
      // REPORTED state, printed here so a reader never has to notice an empty
      // `covered`/`not_covered` pair to learn it — never silently a pass.
      process.stderr.write(
        `combat plan: floor gate is UNBOUND (examined 0) — ${combatPlan.floorGate.binding.reason}\n`,
      );
    }
    if (combatPlan.actorsGate?.unbound) {
      process.stderr.write(
        `combat plan: actor gate is UNBOUND (examined 0) — ${combatPlan.actorsGate.reason}\n`,
      );
    }
  }

  try {
    let failure: unknown;
    try {
      await withTimeout(
        (async () => {
          crashStage = "connect";
          await executor.connect();
          // From here the executor is the authority on which stage a crash is in.
          crashStage = "critical-path";
          await runSequence(criticalPath, executor, {
            retryOnDeath: retryOnDeathFromEnv(),
          });
          // Only after the path is proven. The death loop deliberately
          // kills the player, so running it earlier would leave every later step
          // walking out of a grave — and a delve whose critical path is broken
          // has a bigger finding than its recovery stake.
          if (deathLoop) await executor.runDeathLoop();
        })(),
        budgetMs,
      );
    } catch (err) {
      failure = err;
    }

    // The report is written whether the run passed or failed: a red run's assist
    // windows and death trials are exactly what a reader needs to see.
    const trials = executor.deathTrials();
    const assists = executor.assistWindows();
    // Two independent ways the stage can be red: a trial that reached a verdict
    // and failed it, and a trial (or a whole encounter) the run never proved at
    // all. The second is the one an empty `die_retry` array used to hide.
    const dieRetryFailures = [
      // A precondition gap comes FIRST: it explains why the trials below are
      // missing, and reading the coverage failure without it sends a reader
      // hunting a content bug that is not there.
      ...executor.dieRetryPreconditionFindings(),
      ...dieRetryFindings(trials),
      ...(dieRetry
        ? dieRetryCoverageFailures(
            (combatPlan?.encounters ?? []).filter(
              (e) => !executor.dieRetryPreconditionWaves().has(e.wave),
            ),
            executor.dieRetryEngagements(),
            trials,
          )
        : []),
    ];
    // playtest-methodology rule 1, for the stage that most needed it. Measured
    // 2026-08-11: NO campaign or fixture in either repo exercises a scripted
    // death, and every one of those runs reported this stage green. A green that
    // examined nothing is vacuous, and the only way a reader learns that today is
    // by noticing an empty `die_retry` array — which is exactly how the island's
    // combat floor gate examined zero enemies for nineteen rounds.
    const retryBinding = dieRetryBinding(
      dieRetry,
      combatPlan?.encounters.length ?? 0,
      executor.dieRetryEngagements(),
      trials,
      executor.dieRetryPreconditionAdvisories().length,
      executor.dieRetryPreconditionFindings().length,
    );
    if (retryBinding.unbound) {
      process.stderr.write(
        `die-retry: stage is UNBOUND (0 scripted deaths of ${retryBinding.declared} declared ` +
          `encounter(s)) — ${retryBinding.reason ?? "no reason given"}\n`,
      );
    }
    const leaked = executor.leakedAssists();
    report.recordAssists(assists);
    report.recordTrials(trials);
    // Per-encounter assist policy + how far the run got, so a reader can tell a
    // policy-empty assist ledger from an unwired one.
    const encounterReports: EncounterReport[] = (combatPlan?.encounters ?? []).map(
      (enc): EncounterReport => ({
        encounter: enc.objective,
        wave: enc.wave,
        tier: enc.tier,
        assistPolicy: assistPolicy(enc),
        phaseReached: executor.encounterPhase(enc.wave),
        assistWindows: assists.filter((w) => w.wave === enc.wave).length,
        attribution: executor.waveAttribution(enc.wave),
      }),
    );
    report.recordEncounters(encounterReports);
    // The compiler's floor-gate ledger, verbatim, plus one row per tiered
    // actor — fought (with the outcome) or not (with the reason). Recorded even on
    // a red run: what the gate could NOT measure is exactly what a reader of a
    // failed run needs, and an actor the run never reached must still be visible.
    if (combatPlan) {
      const actorTrials = executor.actorFightTrials();
      const actorReports: ActorReport[] = combatPlan.actors.map((a): ActorReport => {
        const trial = actorTrials.find((t) => t.actor === a.actor);
        if (trial) {
          return {
            actor: a.actor,
            tier: a.tier,
            entity: a.entity,
            anchor: a.anchor,
            covered: a.floorGate.covered,
            exercised: true,
            trial,
          };
        }
        const decision = actorExercise(a, pathObjectives);
        return {
          actor: a.actor,
          tier: a.tier,
          entity: a.entity,
          anchor: a.anchor,
          covered: a.floorGate.covered,
          exercised: false,
          reason:
            decision.kind === "skip"
              ? decision.reason
              : actorFloor
                ? `unleashed by ${decision.afterObjective}, which this run never reached`
                : "skipped via DELVEWRIGHT_ACTOR_FLOOR=0",
        };
      });
      report.recordCombatCoverage(combatPlan.floorGate, actorReports);
      report.recordActorsGate(combatPlan.actorsGate);
    }
    // spec-0029: the name-preference binding, always recorded — including a zero.
    report.recordNamePreference(executor.namePreference());
    report.recordRests(executor.performedRests());
    // Reclassify, never suppress (2026-08-06 island triage): a `despawn-actor
    // style: vanish` broadcasts the same "<name> died" line a real combat loss
    // does, and this run has no wired `min_y` to derive an exact depth cutoff
    // from — see teardown.ts for the fallback heuristic.
    report.recordNamedEntityDeaths(classifyNamedEntityDeaths(executor.namedEntityDeaths()));
    for (const f of executor.floorGateFindings()) report.recordFloorFinding(f);
    for (const f of executor.unkillableFindings()) report.recordUnkillableFinding(f);
    // spec-0025 §3: every enumerated branch appears here — the one this session
    // walked with its result, and each of the others with the reason it did not.
    // A skipped branch is named, never silent.
    if (branchPlan && selection) {
      const outcomes: BranchOutcome[] = branchPlan.branches.map((b): BranchOutcome => {
        if (driven !== undefined && b.id === driven.id) {
          return {
            branch: b.id,
            ran: true,
            passed: failure === undefined,
            pathFile: b.pathFile,
            chronicle: b.chronicle,
            entryCommands,
            endings: b.endings,
          };
        }
        const skipped = selection.skipped.find((s) => s.branch === b.id);
        const reason =
          skipped?.reason ??
          (driven === undefined
            ? `selected by this tier, but no branch was driven (DELVEWRIGHT_BRANCH unset): ` +
              `this session walked the exported critical path`
            : `selected by this tier; a branch run needs a fresh world, so it runs in its ` +
              `own session (validation/branch-runs.sh)`);
        return {
          branch: b.id,
          ran: false,
          passed: false,
          reason,
          chronicle: b.chronicle,
          entryCommands: [],
          endings: b.endings,
        };
      });
      report.recordBranches(selection.tier, driven?.id, outcomes);
      report.stage({
        stage: "branch-run",
        ran: driven !== undefined,
        passed: driven !== undefined && failure === undefined,
        findings: [
          ...(waypointFinding === undefined ? [] : [waypointFinding]),
          ...(driven === undefined
            ? [
                "this build declares narrative branches and none was driven " +
                  "(DELVEWRIGHT_BRANCH unset) — the run proves the exported path only",
              ]
            : []),
        ],
        failures: [],
      });
    }
    report.stage({
      stage: "critical-path",
      ran: true,
      passed: failure === undefined,
      findings: leaked.map(
        (w) => `assist window on ${w.wave} was never closed — harness bug, not content`,
      ),
      failures:
        failure === undefined
          ? []
          : [failure instanceof Error ? failure.message : String(failure)],
    });
    // The death loop. Recorded whether it ran or not, and a stage that
    // did not run carries the reason: a skipped stage must never be readable as a
    // passed one, and this stage is the ONLY runtime proof of the mechanic the
    // whole souls shape rests on.
    const lethalTrials = executor.deathLoopTrials();
    const deathBinding = deathPlan ? deathLoopBinding(deathPlan, lethalTrials) : undefined;
    // The stage runs only AFTER the path is proven, so a run that died on the path
    // never reached it. Reporting that as a death-loop failure would blame this
    // stage for a fault upstream of it — the mirror of the "skipped read as
    // passed" error, and just as misleading to whoever reads the report.
    const deathLoopRan = deathLoop && failure === undefined;
    const deathLoopFailures = deathLoopRan
      ? [
          ...(deathBinding ? deathLoopBindingFailures(deathBinding) : []),
          ...lethalTrials.flatMap((t) => lethalTrialFailures(t)),
        ]
      : [];
    if (deathBinding) report.recordDeathLoop(deathBinding, lethalTrials);
    report.stage({
      stage: "death-loop",
      ran: deathLoopRan,
      passed: deathLoopRan && deathLoopFailures.length === 0,
      findings: deathLoopRan
        ? executor.deathLoopSkipReason() === undefined
          ? []
          : [`the stage stopped before entering any volume — ${executor.deathLoopSkipReason()}`]
        : deathLoop
          ? [
              "the critical path failed, so the death loop was never reached — nothing " +
                "about dying is proven or disproven by this run",
            ]
          : [
            deathPlan === undefined
              ? "no death plan in this build — the campaign declares no lethal volume, no " +
                "`on_death` and no recovery stake, so there is no death loop to prove"
              : deathPlan.binding.unbound
                ? `this build's death plan is UNBOUND (${deathPlan.binding.reason ?? "no reason given"}) ` +
                  `— nothing about dying is proven at runtime by this run`
                : "skipped via DELVEWRIGHT_DEATH_LOOP=0",
          ],
      failures: deathLoopFailures,
    });
    report.recordDieRetryBinding(retryBinding);
    report.stage({
      stage: "die-retry",
      ran: dieRetry,
      passed: dieRetry && dieRetryFailures.length === 0,
      findings: dieRetry
        ? // A zero binding is a FINDING, not a pass (playtest-methodology rule 1).
          // Advisory rather than red on purpose: whether a campaign owes a
          // checkpoint before a fight is `DW0379`/`DW0315`/`DW0316`'s judgement,
          // not this stage's — but a stage that scripted no death may never read
          // as one that proved dying is safe.
          [
            ...(retryBinding.unbound
              ? [
                  `die-retry examined ZERO scripted deaths across ` +
                    `${retryBinding.declared} declared encounter(s): ` +
                    `${retryBinding.reason ?? "no reason given"}`,
                ]
              : []),
            // An encounter the campaign fires NO checkpoint before has its
          // scripted death skipped, and the stage says so out loud rather than
          // passing quietly. Advisory, not a failure — every death there is a full
          // restart, which is a content staging fact the compiler's retry-cost and
            // checkpoint rules judge, not this stage.
            ...executor.dieRetryPreconditionAdvisories(),
          ]
        : [
            combatPlan === undefined
              ? "no combat plan in this build — the campaign declares no mandatory combat"
              : "skipped via DELVEWRIGHT_DIE_RETRY=0",
          ],
      failures: dieRetryFailures,
    });

    const reportPath = reportPathFromEnv();
    if (reportPath) {
      await writeRunReport(reportPath, report);
      process.stderr.write(`run report written to ${reportPath}\n`);
    }
    for (const finding of report.findings()) {
      process.stderr.write(`[finding] ${finding}\n`);
    }

    if (failure !== undefined) throw failure;
    // A die-retry failure is a red run in its own right: the delve may be
    // completable and still ship a broken retry loop, which is the one thing a
    // souls delve cannot do.
    if (dieRetryFailures.length > 0) {
      throw new Error(
        `die-retry stage FAILED (${dieRetryFailures.length} finding(s)):\n` +
          dieRetryFailures.map((f) => `  ${f}`).join("\n"),
      );
    }
    // A delve can be completable and still ship a broken death loop —
    // which, for a souls-shaped delve, is the whole game. Red in its own right.
    if (deathLoopFailures.length > 0) {
      throw new Error(
        `death-loop stage FAILED (${deathLoopFailures.length} finding(s)):\n` +
          deathLoopFailures.map((f) => `  ${f}`).join("\n"),
      );
    }
    process.stderr.write(
      `${driven === undefined ? "critical path" : `branch ${driven.id}`} ` +
        `'${criticalPath.campaignId}' PASSED (${criticalPath.steps.length} steps` +
        `${trials.length > 0 ? `, ${trials.length} scripted death(s) survived` : ""}` +
        `${report.findings().length > 0 ? `, ${report.findings().length} advisory finding(s)` : ""})\n`,
    );
    return 0;
  } finally {
    executor.close();
  }
}

main()
  .then((code) => {
    process.exit(code);
  })
  .catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`FAILED: ${message}\n`);
    process.exit(isBotDeath(err) ? EXIT_BOT_DEATH : 1);
  });
