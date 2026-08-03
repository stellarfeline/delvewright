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
import { loadWaypointsForCriticalPath } from "./waypoints.ts";
import {
  assistPolicy,
  dieRetryCoverageFailures,
  dieRetryFindings,
  loadCombatPlanForCriticalPath,
} from "./combat.ts";
import {
  RunReport,
  reportPathFromEnv,
  writeRunReport,
  type BranchOutcome,
  type EncounterReport,
} from "./report.ts";
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

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`run exceeded wall-clock budget of ${ms}ms`));
    }, ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

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
  let text = exported;
  if (drivenId !== undefined) {
    if (branchPlan === undefined || selection === undefined) {
      throw new Error(
        `DELVEWRIGHT_BRANCH=${drivenId} but this build declares no branches ` +
          `(no validation/branch-plan.json beside ${pathArg}) — nothing to drive`,
      );
    }
    driven = resolveDrivenBranch(branchPlan, selection, drivenId);
    const branchPathFile = nodePath.join(branchPlan.dir, driven.pathFile!);
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

  // task #38: if the compiler's proven waypoint artifact accompanies the critical
  // path, the executor navigates each walked leg through it (successive nearby
  // goals) so no single distant A* solve strands the bot on a large open cave.
  // Absent → single-goal navigation (fallback); malformed → hard failure.
  //
  // The artifact's legs are consumed in LOCKSTEP with the walked positions of the
  // EXPORTED path, so they may only be replayed when that is the path being
  // walked. A branch whose path differs walks without them (single-goal
  // fallback) — per-branch waypoints are a compiler-side follow-up, and using the
  // wrong legs would strand the bot while looking like a content fault.
  const walksExportedPath = text === exported;
  const waypoints = walksExportedPath ? await loadWaypointsForCriticalPath(pathArg) : undefined;
  const waypointFinding =
    !walksExportedPath && (await loadWaypointsForCriticalPath(pathArg)) !== undefined
      ? `branch ${driven?.id ?? "?"} walked without the compiler's waypoint artifact: it is ` +
        `exported for the critical path only, and its legs are position-ordered ` +
        `(per-branch waypoints are a compiler follow-up)`
      : undefined;

  // compiler #220: the path's rest steps, with their EXPORTED indices. The bot
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
  const combatPlan = await loadCombatPlanForCriticalPath(pathArg);
  const dieRetry = combatPlan !== undefined && dieRetryFromEnv();
  const report = new RunReport(criticalPath.campaignId, combatPlan?.difficulty ?? "unknown");
  if (combatPlan) {
    executor.useCombatPlan(combatPlan, dieRetry);
    process.stderr.write(
      `combat plan: ${combatPlan.encounters.length} mandatory encounter(s) at ` +
        `difficulty '${combatPlan.difficulty}'; die-retry ${dieRetry ? "ON" : "SKIPPED"}\n`,
    );
  }

  try {
    let failure: unknown;
    try {
      await withTimeout(
        (async () => {
          await executor.connect();
          await runSequence(criticalPath, executor, {
            retryOnDeath: retryOnDeathFromEnv(),
          });
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
    const leaked = executor.leakedAssists();
    report.recordAssists(assists);
    report.recordTrials(trials);
    // Per-encounter assist policy + how far the run got, so a reader can tell a
    // policy-empty assist ledger from an unwired one (task #102).
    const encounterReports: EncounterReport[] = (combatPlan?.encounters ?? []).map(
      (enc): EncounterReport => ({
        encounter: enc.objective,
        wave: enc.wave,
        tier: enc.tier,
        assistPolicy: assistPolicy(enc),
        phaseReached: executor.encounterPhase(enc.wave),
        assistWindows: assists.filter((w) => w.wave === enc.wave).length,
      }),
    );
    report.recordEncounters(encounterReports);
    report.recordRests(executor.performedRests());
    for (const f of executor.floorGateFindings()) report.recordFloorFinding(f);
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
    report.stage({
      stage: "die-retry",
      ran: dieRetry,
      passed: dieRetry && dieRetryFailures.length === 0,
      findings: dieRetry
        ? []
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
